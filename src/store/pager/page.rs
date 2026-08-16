use std::io::Read;

use crate::{
    errors::{StoreResult, StoreErr},
};

use super::{ read_usize, read_u16, PageId};

pub const PAGE_SIZE: usize = 4096;
pub trait Page: Sized {
    fn header(&self) -> &PageHeader;
    fn pagetype() -> PageType;
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(header: PageHeader, bytes: &mut &[u8]) -> StoreResult<Self>;
}

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

pub struct Slot {
    pub offset: usize,
    pub len: usize
}

impl Slot {
    pub fn read<R: Read>(bytes: &mut R) -> StoreResult<Self> {
        let offset = read_usize(bytes)?;
        let len = read_usize(bytes)?;
        Ok(Self { offset, len })
    }
}
