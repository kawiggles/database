use super::{
    Page, PageId, PageType, PageHeader,
    page::{PageCursor, PAGEHEADER_SIZE, PAGE_SIZE},
    read_usize
};

use crate::{
    errors::{StoreResult, StoreErr},
    store::{ Value, Rid },
};

pub struct DataPage {
    header: PageHeader,
    data: Vec<Vec<u8>>,
    overflow: Option<PageId>,
}

impl DataPage {
    pub fn new() -> Self {
        todo!()
    }
    
    pub fn insert(&mut self, val: Value) -> StoreResult<Rid> {
        let page = self.header.id;
        let slot = (self.header.slots + 1) as usize;

        let bytes = val.to_bytes();

        self.header.upper -= (bytes.len() + 2) as u16;
        self.header.lower += 4;
        self.header.slots += 1;
        self.data.push(bytes);

        Ok(Rid { page, slot })
    }
}

impl Page for DataPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype() -> PageType {
        PageType::Data
    }

    fn free_space(&self) -> usize {
        (self.header.upper - self.header.lower) as usize
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for data in &self.data {
            let offset = end - data.len();

            if offset < dir + 4 {
                return Err(StoreErr::SlotOverwrite {
                    page: self.header.id,
                    len: data.len(),
                    pagetype: PageType::Data,
                });
            }

            bytes[offset..offset].clone_from_slice(&data);
            end = offset;

            bytes[dir..dir+2].clone_from_slice(&(offset as u16).to_le_bytes());
            bytes[dir+2..dir+4].clone_from_slice(&(data.len() as u16).to_le_bytes());
            dir += 4
        }

        if end - 8 < dir + 4 {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: 8,
                pagetype: PageType::Data,
            });
        }

        let next = self.overflow
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[end-8..end].copy_from_slice(&next);
        bytes[dir..dir+2].copy_from_slice(&((end - 8) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(8 as u16).to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut data: Vec<Vec<u8>> = Vec::new();

        for _ in 0..header.slots - 1 { data.push(cursor.next()?.to_vec()); }

        let overflow = PageId::new(read_usize(&mut cursor.next()?)?);
        
        Ok(Self { header, data, overflow })
    }
}

