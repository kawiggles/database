use std::{
    num::NonZeroUsize,
};

use crate::{
    errors::{StoreResult},
};

use super::Oid;

pub const PAGE_SIZE: usize = 4096;

pub trait Page: Sized {
    fn header(&self) -> &PageHeader;
    fn pagetype(&self) -> PageType;
    fn next(&self) -> Option<PageId>;
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(bytes: [u8; PAGE_SIZE]) -> StoreResult<Self>;
}

pub struct PageHeader {
    pub table_oid: Oid,
    pagetype: PageType,
    next: Option<PageId>,
    pub slots: u16,
    pub lower: u16, // start of data slots
    pub upper: u16, // end of pointer slots
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub struct PageId(NonZeroUsize);

impl PageId {
    fn new(offset: usize) -> Option<Self> {
        NonZeroUsize::new(offset).map(PageId)
    }

    fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Copy, Clone)]
pub enum PageType {
    Branch,
    Leaf,
    Data,
    Overflow,
}

pub struct BranchPage {
    header: PageHeader,
    keys: Vec<PageId>,
    children: Vec<PageId>,
}

impl BranchPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for BranchPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype(&self) -> PageType {
        self.header.pagetype
    }

    fn next(&self) -> Option<PageId> {
        self.header.next
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: [u8; PAGE_SIZE]) -> StoreResult<Self> {
        todo!()
    }
}

pub struct LeafPage {
    header: PageHeader,
    pages: Vec<PageId>,
    next_leaf: Option<PageId>
}

impl LeafPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for LeafPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype(&self) -> PageType {
        self.header.pagetype
    }

    fn next(&self) -> Option<PageId> {
        self.header.next
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: [u8; PAGE_SIZE]) -> StoreResult<Self> {
        todo!()
    }
}

pub struct DataPage {
    header: PageHeader,
    overflow: Option<PageId>,
}

impl DataPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for DataPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype(&self) -> PageType {
        self.header.pagetype
    }

    fn next(&self) -> Option<PageId> {
        self.header.next
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: [u8; PAGE_SIZE]) -> StoreResult<Self> {
        todo!()
    }
}

pub struct OverflowPage {
    header: PageHeader,
    data: Vec<u8>,
    next: Option<PageId>,
}

impl OverflowPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for OverflowPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype(&self) -> PageType {
        self.header.pagetype
    }

    fn next(&self) -> Option<PageId> {
        self.header.next
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: [u8; PAGE_SIZE]) -> StoreResult<Self> {
        todo!()
    }
}
