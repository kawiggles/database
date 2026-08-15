use std::{
    fs::{File},
    io::{Read, Seek, SeekFrom},
    num::NonZeroUsize,
};

use crate::{
    errors::{StoreResult, StoreErr},
};

use super::{Oid, read_usize, read_u16};

pub const PAGE_SIZE: usize = 4096;
pub trait Page: Sized {
    fn header(&self) -> &PageHeader;
    fn pagetype() -> PageType;
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self>;
}

pub struct PageHeader {
    pub table_oid: Oid,         // 8
    pub pagetype: PageType,     // 8
    pub next: Option<PageId>,   // 8
    pub slots: u16,             // 2
    pub lower: u16,             // 2
    pub upper: u16,             // 2
}

impl PageHeader {
    pub fn read(id: PageId, file: &mut File) -> StoreResult<Self> {
        file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64))?;

        let table_oid = Oid(read_usize(file)?);

        let pagetype = PageType::deserialize(file)?;

        let next = PageId::new(read_usize(file)?);
        let slots = read_u16(file)?;
        let lower = read_u16(file)?;
        let upper = read_u16(file)?;
        
        Ok(PageHeader { table_oid , pagetype, next, slots, lower, upper })
    }

    pub fn write(file: &mut File, id: &PageId) -> StoreResult<()> {
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub struct PageId(pub NonZeroUsize);

impl PageId {
    pub fn new(offset: usize) -> Option<Self> {
        NonZeroUsize::new(offset).map(PageId)
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PageType {
    Free,
    Branch,
    Leaf,
    Data,
    Overflow,
}

impl PageType {
    fn serialize(&self) -> u8 {
        match self {
            Self::Free => 0,
            Self::Branch => 1,
            Self::Leaf => 2,
            Self::Data => 3,
            Self::Overflow => 4,
        }
    }

    fn deserialize(file: &mut File) -> StoreResult<Self> {
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf)?;
        
        match u8::from_le_bytes(buf) {
            0 => Ok(Self::Free),
            1 => Ok(Self::Branch),
            2 => Ok(Self::Leaf),
            3 => Ok(Self::Data),
            4 => Ok(Self::Overflow),
            n => Err(StoreErr::UnknownPagetype(n))
        }
    }
}

pub struct BranchPage {
    header: PageHeader,
    keys: Vec<String>,
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

    fn pagetype() -> PageType {
        PageType::Branch
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self> {
        file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64))?;
        let header = PageHeader::read(*id, file)?;
        let keys = ;
        let children = ;

        Ok( Self { header, keys, children })
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

    fn pagetype() -> PageType {
        PageType::Leaf
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self> {
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

    fn pagetype() -> PageType {
        PageType::Data
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self> {
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

    fn pagetype() -> PageType {
        PageType::Overflow
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self> {
        todo!()
    }
}

pub struct FreePage(PageHeader);

impl Page for FreePage {
    fn header(&self) -> &PageHeader {
        &self.0
    }

    fn pagetype() -> PageType {
        PageType::Free
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(file: &mut File, id: &PageId) -> StoreResult<Self> {
        todo!()
    }
}
