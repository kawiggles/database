use super::{
    Page, PageId, PageType, PageHeader,
    page::{PageCursor, PAGEID_SIZE, PAGEHEADER_SIZE, PAGE_SIZE, SLOT_POINTER_SIZE},
    read_usize
};

use crate::{
    errors::{StoreErr, StoreResult}, store::Rid,
};

pub struct DataPage {
    header: PageHeader,
    data: Vec<Option<Vec<u8>>>,
    overflow: Option<PageId>,
}

impl DataPage {
    pub fn new(id: PageId) -> Self {
        let header = PageHeader {
            id,
            pagetype: PageType::Data,
            next: None,
            slots: 1 as u16,
            lower: PAGEHEADER_SIZE as u16,
            upper: PAGE_SIZE as u16,
        };

        Self { header, data: Vec::new(), overflow: None }
    }
    
    pub fn get(&self, slot: u16) -> StoreResult<Vec<u8>> {
        match self.data.get(slot as usize).and_then(|o| o.as_deref()) {
            Some(x) => Ok(x.to_vec()),
            None => Err(StoreErr::DatapageNoSlotData { page: self.header.id, slot })
        }
    }

    pub fn insert(&mut self, bytes: &[u8]) -> StoreResult<Rid> {
        self.data.push(Some(bytes.to_vec()));

        self.refresh_header();
        // TODO: overflow logic
        Ok(Rid { page: self.header.id, slot: self.data.len() as u16 - 1 })
    }
    
    pub fn delete(&mut self, slot: u16) -> StoreResult<Vec<u8>> {
        todo!()
    }

    fn refresh_header(&mut self) {
        self.header.slots = self.data.len() as u16 + 1;
        self.header.lower = PAGEHEADER_SIZE as u16 + self.header.slots * SLOT_POINTER_SIZE as u16;

        let sum = self.data
            .iter()
            .flatten()
            .map(|b| b.len())
            .sum::<usize>();

        self.header.upper = PAGE_SIZE
            .checked_sub(sum)
            .and_then(|v| v.checked_sub(PAGEID_SIZE))
            .unwrap_or(0) as u16;

        debug_assert_eq!(self.data.len() + 1, self.header.slots as usize);
    }
}

impl Page for DataPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn set_id(&mut self, id: PageId) {
        self.header.id = id;
    }

    fn pagetype() -> PageType {
        PageType::Data
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for data_o in &self.data {
            let Some(data) = data_o else { 
                if end < dir + SLOT_POINTER_SIZE {
                    return Err(StoreErr::SlotOverwrite {
                        page: self.header.id,
                        len: 0,
                        pagetype: PageType::Data,
                    });
                }

                bytes[dir..dir+4].clone_from_slice(&[0u8; 4]);
                dir += SLOT_POINTER_SIZE;
                continue;
            };
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
            dir += SLOT_POINTER_SIZE;
        }

        if end - PAGEID_SIZE < dir + SLOT_POINTER_SIZE {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: SLOT_POINTER_SIZE,
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
        let mut data: Vec<Option<Vec<u8>>> = Vec::new();

        // check here for off-by-one errors
        for _ in 1..header.slots { 
            let dead = cursor.offset()? == 0;
            let bytes = cursor.next()?;
            data.push((!dead).then(|| bytes.to_vec()));
        }

        let overflow = PageId::new(read_usize(&mut cursor.next()?)?);
        
        Ok(Self { header, data, overflow })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_and_deserialize() {
        let mut data = DataPage::new(PageId::new(1).unwrap());
        let bytes = vec![b'h', b'i'];
        data.insert(&bytes).unwrap();
    }

    #[test]
    fn get_slot() {
    }
}
