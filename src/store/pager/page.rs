use std::{
    io::Read,
    num::NonZeroUsize,
    fmt::{Display, Formatter, Result},
};

use crate::errors::{StoreResult, StoreErr};

use super::{ read_usize, read_u16 };

pub const SLOT_POINTER_SIZE: usize = 4; // u16 + u16
pub const PAGE_SIZE: usize = 4096;
pub trait Page: Sized {
    fn header(&self) -> &PageHeader;
    fn free_space(&self) -> Option<u16>;
    fn pagetype() -> PageType;
    fn serialize(&self) -> StoreResult<Vec<u8>>;
    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self>;
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

impl Display for PageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0)
    }
}

pub const PAGEHEADER_SIZE: usize = 30;
pub struct PageHeader {
    pub id: PageId,             // 8
    pub pagetype: PageType,     // 8
    pub next: Option<PageId>,   // 8
    pub slots: u16,             // 2
    pub lower: u16,             // 2
    pub upper: u16,             // 2
}

impl PageHeader {
    pub fn deserialize(bytes: &mut &[u8]) -> StoreResult<Self> {
        let id = PageId::new(read_usize(bytes)?)
            .expect("read PageId of 0");

        let pagetype = PageType::deserialize(bytes)?;

        let next = PageId::new(read_usize(bytes)?);
        let slots = read_u16(bytes)?;
        let lower = read_u16(bytes)?;
        let upper = read_u16(bytes)?;
        
        Ok(PageHeader { id, pagetype, next, slots, lower, upper })
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();

        bytes.extend_from_slice(&self.id.get().to_le_bytes());
        bytes.push(self.pagetype.serialize());
        bytes.extend_from_slice(&self.next
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes());
        bytes.extend_from_slice(&self.slots.to_le_bytes());
        bytes.extend_from_slice(&self.lower.to_le_bytes());
        bytes.extend_from_slice(&self.upper.to_le_bytes());
        bytes
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

    fn deserialize<R: Read>(bytes: &mut R) -> StoreResult<Self> {
        let mut buf = [0u8; 1];
        bytes.read_exact(&mut buf)?;
        
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

pub struct PageCursor<'a> {
    page: &'a [u8],
    pos: usize,
}

impl<'a> PageCursor<'a> {
    pub fn new(page: &'a [u8]) -> Self {
        Self { page, pos: PAGEHEADER_SIZE }
    }

    pub fn next(&mut self) -> StoreResult<&'a [u8]> {
        let mut entry = &self.page[self.pos..self.pos + 4]; // u16 + u16 for 4 bytes total
        let offset = read_u16(&mut entry)? as usize;
        let len = read_u16(&mut entry)? as usize;
        self.pos += 4;
        self.page.get(offset..offset+len).ok_or(StoreErr::SlotOOB { offset, len })
    }
}
