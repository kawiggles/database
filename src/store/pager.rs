use crate::store::bptree::Node;
use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::{File};

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

pub struct PageId(usize);

pub enum Page {
    Index(IndexPage),
    Data(DataPage),
}

#[derive(Encode, Decode)]
pub struct IndexPage {
    page_id: PageId,
    node: Node,
}

#[derive(Encode, Decode)]
pub struct DataPage {
    page_id: PageId,
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

pub struct Pager {
    file: File,
    cache: HashMap<PageId, Node>,
    free_list: HashSet<PageId>,
    num_pages: usize,
}

impl Pager {
    pub fn open(path: &str) -> Result<Self, DbError> {
    }

    pub fn read(&self, id: PageId) -> Result<Page, DbError> {
    }

    pub fn write(&mut self, id: PageId) -> Result<Page, DbError> {
    }

    pub fn alloc(&mut self) -> PageId) {
    }
}
