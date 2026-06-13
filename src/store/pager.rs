use crate::VERSION;
use crate::store::{DEFAULT_ORDER, DEFAULT_FILE};
use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom, BufReader};
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

// TODO: Make data representation more efficient (manual bincode)
#[derive(Encode, Decode)]
pub struct PageHeader {
    page_type: PageType, // 4 bytes
    page_id: PageId, // 8 bytes
    next_free: Option<PageId>, // 8 bytes (Option) + 8 bytes
} // Total: 28 bytes

pub fn write_page(file: &mut File, id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), DbError> {
    file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
    file.write_all(buf)?;
    Ok(())
}

#[derive(Encode, Decode)]
pub struct IndexPage {
    pub header: PageHeader, // 28 bytes
    pub keys: Vec<String>, // 8 + ? Bytes
    pub node_type: NodeType, // 28 + ? Bytes
}

impl IndexPage {
    fn write(file: &mut File, new_page: &IndexPage) -> Result<(), DbError> {
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(&new_page, &mut page, INDEX_CONFIG)?;
        write_page(file, new_page.header.page_id, &page)?;
        Ok(())
    }

    pub fn read(file: &mut File, id: PageId) -> Result<IndexPage, DbError> {
        let mut buf = [0u8; PAGE_SIZE];
        file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
        file.read_exact(&mut buf)?;
        let (page, size): (IndexPage, usize) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(DbError::ReadOverflow)
        }

        Ok(page)
    }
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

impl DataPage {
    fn write(file: &mut File, new_page: DataPage) -> Result<(), DbError> {
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(&new_page, &mut page, DATA_CONFIG)?;
        write_page(file, new_page.header.page_id, &page)?;
        Ok(())
    }

    pub fn read(file: &mut File, id: PageId) -> Result<DataPage, DbError> {
        let mut buf = [0u8; PAGE_SIZE];
        file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
        file.read_exact(&mut buf)?;
        let (page, size): (DataPage, usize) = bincode_next::decode_from_slice(&buf, DATA_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(DbError::ReadOverflow)
        }

        Ok(page)
    }
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

impl DbHeader {
    fn write(&self, file: &mut File) -> Result<(), DbError> {
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(self, &mut page, INDEX_CONFIG)?;
        write_page(file, PageId(0), &page)?;
        Ok(())
    }
}

pub struct Pager {
    file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, IndexPage>,
    pub num_pages: usize,
}

#[derive(Decode)]
struct FreeListReader {
    _page_type: PageType,
    _page_id: PageId,
    next_free: Option<PageId>,
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
            num_pages: 1,
            free_list_head: None,
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(DEFAULT_FILE)?;
        new_head.write(&mut file)?;

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
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
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
            let (reader, _): (FreeListReader, _) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;
            current = reader.next_free;
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
        for (_, page) in &mut self.dirty_cache.drain() {
            IndexPage::write(&mut self.file, &page)?;
        }
        Ok(())
    }

    // Construct page and serialize it
    pub fn alloc(&mut self) -> PageId {
        if self.free_list.is_empty() {
            let id = PageId(self.num_pages + 1);
            self.num_pages += 1;
            id
        } else {
            self.free_list.pop().unwrap()
        }
    }
}
