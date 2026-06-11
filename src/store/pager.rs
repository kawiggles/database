use crate::VERSION;
use crate::store::{DEFAULT_ORDER, DEFAULT_FILE};
use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::File;
use std::io::{Write, BufWriter, BufReader};
use std::fmt;
use std::collections::HashMap;

pub const PAGE_SIZE: usize = 4096;

const INDEX_CONFIG: config::Configuration<config::BigEndian, config::Fixint, config::Limit<4096>> = 
    config::standard()
    .with_big_endian()
    .with_fixed_int_encoding()
    .with_limit::<PAGE_SIZE>();

const DATA_CONFIG: config::Configuration<config::BigEndian, config::Varint, config::Limit<4000>> = 
    config::standard()
    .with_big_endian()
    .with_limit::<4000>();

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

#[derive(Encode, Decode)]
pub struct PageHeader {
    page_type: PageType,
    page_id: PageId,
}

#[derive(Encode, Decode)]
pub struct IndexPage {
    pub header: PageHeader,
    pub keys: Vec<String>,
    pub node_type: NodeType,
}

#[derive(Encode, Decode)]
pub enum NodeType {
    Branch { children: Vec<PageId> },
    Leaf { pages: Vec<PageId>, next: Option<PageId> }
}

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

// TODO: write logs...
impl Pager {
    // Function to create a new database file if none exists
    pub fn new() -> Result<(Self, Option<PageId>, usize), DbError> {
        // TODO: add way to change database file name and path
        let mut page = [0u8; PAGE_SIZE];
        let new_head = DbHeader{
            magic: MAGIC,
            version: VERSION,
            root_page: None, // None means no root
            // TODO: add way to change db order
            order: DEFAULT_ORDER,
            num_pages: 0,
            free_list_head: None,
        };

        let mut file = File::create(DEFAULT_FILE)?;
        {
            let mut writer = BufWriter::new(&mut file);
            bincode_next::encode_into_slice(&new_head, &mut page, INDEX_CONFIG)?;
            writer.write_all(&page)?;
            writer.flush()?;
        }

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
            // TODO: actually write this function
            let next = somefunctiontoreadpages();
            current = next;
        }

        Ok((Pager {
            file: file,
            free_list: free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }, header.root_page, header.order))
    }

    /*
    // Read a DataPage, Datapage only
    pub fn read(&self, id: &PageId) -> Result<Value, DbError> {
        return Ok(Value::Null);
    }

    // Write to a DataPage, DataPage only
    pub fn write(&mut self, id: PageId, val: Value) -> Result<Value, DbError> {
        return Ok(Value::Null);
    }

    // Read node metadata, for IndexPage only
    pub fn peek(&self, id: PageId) -> IndexPage {
    }

    // Modify node metadata, for IndexPage only
    pub fn poke(&mut self, id: PageId) -> &mut IndexPage {
        // Ensure to write to dirty cache
    }
    
    // Clear out the cache and write it to disk
    fn flush(&mut self) -> Result<(), DbError> {
    }

    // Construct page and serialize it
    pub fn alloc(&mut self, page_type: PageType) -> PageId {
        match page_type {
            Index => {
                new_page = 
            },
            Data => {
            },
        }
    }
    */
}
