use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::{File};
use std::fmt;
use std::collections::HashMap;
use std::collections::HashSet;

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
    root_page: PageId,
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
    free_list: HashSet<PageId>,
    dirty_cache: HashMap<PageId, CachedPage>,
    pub num_pages: usize,
}

impl Pager {
    /*
    pub fn open(path: &str) -> Result<Self, DbError> {
    }

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
