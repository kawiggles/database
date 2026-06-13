use crate::VERSION;
use crate::store::{DEFAULT_ORDER, DEFAULT_FILE};
use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom, BufWriter, BufReader};
use std::fmt;
use std::collections::HashMap;

pub const PAGE_SIZE: usize = 4096;

const INDEX_CONFIG: config::Configuration<config::BigEndian, config::Fixint, config::Limit<4096>> = 
    config::standard()
    .with_big_endian()
    .with_fixed_int_encoding()
    .with_limit::<PAGE_SIZE>();

const DATA_CONFIG: config::Configuration<config::BigEndian, config::Varint, config::Limit<4096>> = 
    config::standard()
    .with_big_endian()
    .with_limit::<4096>();

const MAGIC: [u8; 8] = *b"KAWIKADB";

#[derive(PartialEq, Encode, Decode, Clone, Copy)]
pub struct PageId(pub usize);

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Encode, Decode)]
pub enum PageType {
    Index,
    Data,
}

// TODO: Make data representation more efficient
#[derive(Encode, Decode)]
pub struct PageHeader {
    page_type: PageType, // 4 bytes
    page_id: PageId, // 8 bytes
    next_free: Option<PageId>, // 8 bytes (Option) + 8 bytes
} // Total: 28 bytes

#[derive(Encode, Decode)]
pub struct IndexPage {
    pub header: PageHeader, // 28 bytes
    pub keys: Vec<String>, // 8 + ? Bytes
    pub node_type: NodeType, // 28 + ? Bytes
}

#[derive(Encode, Decode)]
pub enum NodeType { // 4 bytes
    Branch { children: Vec<PageId> }, // 8 bytes + ? 8 byte PageIds
    Leaf { 
        pages: Vec<PageId>, // 8 bytes + ? 8 byte PageIds
        next: Option<PageId> // 8 bytes (Option) + 8 bytes
    } // 24 + 8? Bytes
} // max 28 + 8? Bytes

#[derive(Encode, Decode)]
pub struct DataPage {
    header: PageHeader,
    value: Value,
}

#[derive(Encode, Decode)]
pub struct DbHeader {
    magic: [u8; 8],
    version: u32,
    root_page: Option<PageId>,
    order: usize,
    num_pages: usize,
    free_list_head: Option<PageId>,
}

struct CachedPage {
    page: IndexPage,
    dirty: bool,
}

pub struct Pager {
    file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, CachedPage>,
    pub num_pages: usize,
}

trait Page {
    fn read_header(file: &mut File, id: PageId) -> Result<PageHeader, DbError>;
    fn write(&self, file: &mut File, id: PageId) -> Result<(), DbError>;
}

fn read_page<T: Decode<()>>(file: &mut File, id: PageId) -> Result<T, DbError> {
    let mut buf = [0u8; PAGE_SIZE];
    file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
    file.read_exact(&mut buf)?;
    let (page, size): (T, usize) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;

    if size > PAGE_SIZE {
        return Err(DbError::ReadOverflow)
    }

    Ok(page)
}

impl Page for IndexPage {
    fn read_header(file: &mut File, id: PageId) -> Result<PageHeader, DbError> {
        let page: IndexPage = read_page(file, id)?;
        Ok(page.header)
    }

    fn write(&self, file: &mut File, id: PageId) -> Result<(), DbError> {
    }
}

impl Page for DataPage {
    fn read_header(file: &mut File, id: PageId) -> Result<PageHeader, DbError> {
        let page: DataPage = read_page(file, id)?;
        Ok(page.header)
    }

    fn write(&self, file: &mut File, id: PageId) -> Result<(), DbError> {
    }
}
impl Page for DbHeader {
    fn read_header(_file: &mut File, _id: PageId) -> Result<PageHeader, DbError> {
        Ok(PageHeader {
            page_type: PageType::Index, 
            page_id: PageId(0), 
            next_free: None, 
        })
    }

    fn write(&self, file: &mut File, _id: PageId) -> Result<(), DbError> {
        let mut page = [0u8; PAGE_SIZE];
        let mut writer = BufWriter::new(file);
        bincode_next::encode_into_slice(self, &mut page, INDEX_CONFIG);
        writer.write_all(&page)?;
        writer.flush()?;
        Ok(())
    }
}

// TODO: write logs...
impl Pager {
    // Function to create a new database file if none exists
    pub fn new() -> Result<(Self, Option<PageId>, usize), DbError> {
        // TODO: add way to change database file name and path
        let new_head = DbHeader{
            magic: MAGIC,
            version: VERSION,
            root_page: None, // None means no root
            // TODO: add way to change db order (requires variable page size)
            order: DEFAULT_ORDER,
            num_pages: 0,
            free_list_head: None,
        };

        let mut file = File::create(DEFAULT_FILE)?;
        new_head.write(&mut file, PageId(0));

        Ok((Pager {
            file: file,
            free_list: Vec::new(),
            dirty_cache: HashMap::new(),
            num_pages: 0,
        }, new_head.root_page, new_head.order))
    }

    // Function to open database if one exists
    pub fn open(path: &str) -> Result<(Self, Option<PageId>, usize), DbError> {
        // TODO: if no path, use default file (consider Option<&str>)
        let mut file = File::open(path)?;
        let mut reader = BufReader::new(&mut file);
        let header: DbHeader = bincode_next::decode_from_std_read(&mut reader, INDEX_CONFIG)?;

        if header.magic != MAGIC {
            return Err(DbError::BadFile);
        }

        let mut free_list: Vec<PageId> = Vec::new();
        let mut current = header.free_list_head;
        while let Some(id) = current {
            free_list.push(id);
            let mut buf = [0u8; PAGE_SIZE];
            file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
            file.read_exact(&mut buf)?;
            let (next, _): (Option<PageId>, _) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;
            current = next;
        }

        Ok((Pager {
            file: file,
            free_list: free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }, header.root_page, header.order))
    }

    // Clear out the cache and write it to disk
    fn flush(&mut self) -> Result<(), DbError> {
    }

    // Construct page and serialize it
    pub fn alloc(&mut self, page_type: PageType) -> PageId {
        match page_type {
            Index => {
                if self.free_list.is_empty() {
                    // Create a new page
                } else {
                    // Pull from that page
                }
            },
            Data => {
                if self.free_list.is_empty() {
                    // Create a new page
                } else {
                    // Pull from that page
                }
            },
        }
    }
}
