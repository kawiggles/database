use super::{
    Page, PageId, PageType, PageHeader,
    page::{PageCursor, PAGEHEADER_SIZE, PAGE_SIZE, SLOT_POINTER_SIZE},
    read_usize
};

use crate::{
    errors::{StoreErr, StoreResult},
    store::{ Rid, Value, pager::page::PAGEID_SIZE },
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
    
    pub fn get(&self, slot: u16) -> StoreResult<Value> {
        todo!()
    }

    pub fn insert(&mut self, slot: u16, val: Value) -> StoreResult<()> {
        todo!()
    }
    
    pub fn insert_new(&mut self, val: Value) -> StoreResult<Rid> {
        let page = self.header.id;
        let slot = self.header.slots + 1;

        let bytes = val.to_bytes();

        self.header.upper -= bytes.len() as u16;
        self.header.lower += SLOT_POINTER_SIZE as u16;
        self.header.slots += 1;
        self.data.push(bytes);

        Ok(Rid { page, slot })
    }

    pub fn delete(&mut self, slot: u16) -> StoreResult<Value> {
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

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for data in &self.data {
            let offset = end - data.len();

            if offset < dir + SLOT_POINTER_SIZE {
                return Err(StoreErr::SlotOverwrite {
                    page: self.header.id,
                    len: data.len(),
                    pagetype: PageType::Data,
                });
            }

            bytes[offset..end].clone_from_slice(&data);
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

        bytes[end-PAGEID_SIZE..end].copy_from_slice(&next);
        bytes[dir..dir+2].copy_from_slice(&((end - PAGEID_SIZE) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(PAGEID_SIZE as u16).to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut data: Vec<Vec<u8>> = Vec::new();

        for _ in 1..header.slots { data.push(cursor.next()?.to_vec()); }

        let overflow = PageId::new(read_usize(&mut cursor.next()?)?);
        
        Ok(Self { header, data, overflow })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn serialize() {
    }

    #[test]
    fn deserialize() {
    }

    #[test]
    fn get_slot() {
    }
}
