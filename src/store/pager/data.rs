use super::{
    Page, PageId, PageType, PageHeader, Pager,
    page::{PageCursor, PAGEID_SIZE, PAGEHEADER_SIZE, PAGE_SIZE, SLOT_POINTER_SIZE},
    read_usize
};

use crate::{
    errors::{StoreErr, StoreResult}, store::{pager::OverflowPage, relation::Rid},
};

#[derive(Debug, PartialEq)]
pub struct DataPage {
    header: PageHeader,
    data: Vec<Option<Vec<u8>>>,
    overflow: Option<PageId>,
}

pub const MAX_FREE_SPACE: usize = PAGE_SIZE - PAGEHEADER_SIZE - 2 * SLOT_POINTER_SIZE - PAGEID_SIZE;

impl DataPage {
    pub fn new(id: PageId) -> Self {
        let header = PageHeader {
            id,
            pagetype: PageType::Data,
            slots: 1 as u16,
            lower: (PAGEHEADER_SIZE + SLOT_POINTER_SIZE) as u16,
            upper: (PAGE_SIZE - PAGEID_SIZE) as u16,
        };

        Self { header, data: Vec::new(), overflow: None }
    }

    pub fn get(&self, slot: u16, pager: &mut Pager) -> StoreResult<Vec<u8>> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(self.data
            .get(slot as usize)
            .and_then(|o| o.as_deref())
            .ok_or(StoreErr::DatapageNoSlotData { page: self.header.id, slot })?
        );

        if let Some(oid) = self.overflow {
            let mut id = oid;
            loop {
                let overflow = pager.read::<OverflowPage>(id)?;
                data.extend_from_slice(overflow.get());
                if let Some(oid) = overflow.next() {
                    id = oid;
                } else {
                    break;
                }
            }
        };

        Ok(data)
    }

    pub fn get_overflow(&self) -> Option<PageId> {
        self.overflow
    }

    pub fn insert(&mut self, bytes: &[u8]) -> StoreResult<Rid> {
        self.data.push(Some(bytes.to_vec()));

        self.refresh_header();
        Ok(Rid { page: self.header.id, slot: self.data.len() as u16 - 1 })
    }

    pub fn insert_overflow(&mut self, bytes: &[u8], pager: &mut Pager) -> StoreResult<Rid> {
        let (head, tail) = bytes.split_at(MAX_FREE_SPACE);
        self.data.push(Some(head.to_vec()));

        self.overflow = tail
            .rchunks(MAX_FREE_SPACE)
            .try_fold(None, |next, chunk| -> StoreResult<Option<PageId>> {
                let id = pager.alloc();
                let mut page = OverflowPage::new(id);
                page.fill(chunk);
                if let Some(next) = next {
                    page.link(next);
                }
                pager.write(page)?;
                Ok(Some(id))
            })?;

        self.refresh_header();
        Ok(Rid { page: self.header.id, slot: self.data.len() as u16 - 1 })
    }

    pub fn delete(&mut self, slot: u16, pager: &mut Pager) -> StoreResult<Vec<u8>> {
        let mut data: Vec<u8> = Vec::new();

        data.extend_from_slice(&self.data
            .get_mut(slot as usize)
            .and_then(Option::take)
            .ok_or(StoreErr::DatapageNoSlotData { page: self.header.id, slot })?
        );

        if let Some(mut id) = self.overflow {
            loop {
                let overflow = pager.read::<OverflowPage>(id)?;
                data.extend_from_slice(overflow.get());
                pager.free(id)?;
                if let Some(new_id) = overflow.next() {
                    id = new_id;
                } else {
                    break;
                }
            }
            self.overflow = None;
        }

        while let Some(None) = self.data.last() {
            self.data.pop();
        }

        self.refresh_header();

        Ok(data)
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
    use fastrand;
    use tempfile::NamedTempFile;

    fn setup() -> Pager {
        let file = NamedTempFile::new().unwrap();
        let pager = Pager::new(file.path().to_str().unwrap()).unwrap();
        pager
    }

    // TODO: Finish writing these tests
    #[test]
    fn serialize_and_deserialize() {
        let mut data = DataPage::new(PageId::new(1).unwrap());
        let b = vec![b'h', b'i'];

        data.insert(&b).unwrap();

        let bytes = data.serialize().unwrap();
        let mut cursor = PageCursor::new(&bytes);
        let header = data.header.clone();
        let new_data = DataPage::deserialize(header, &mut cursor).unwrap();

        assert_eq!(data, new_data);
    }

    #[test]
    fn insert_and_get_overflow() {
        let mut pager = setup();

        let mut data = DataPage::new(pager.alloc());
        let mut buf: Vec<u8> = vec![0u8; 8192];
        fastrand::fill(&mut buf);

        let rid = data.insert_overflow(&buf, &mut pager).unwrap();
        pager.write(data).unwrap();
        pager.flush().unwrap();

        let new_data = pager.read::<DataPage>(rid.page).unwrap();
        let new_buf = new_data.get(rid.slot, &mut pager).unwrap();

        assert_eq!(buf, new_buf);
    }

    #[test]
    fn delete() {
        let mut pager = setup();

        let mut data = DataPage::new(pager.alloc());
        let mut buf: Vec<u8> = vec![0u8; 8192];
        fastrand::fill(&mut buf);

        let rid = data.insert_overflow(&buf, &mut pager).unwrap();
        pager.write(data).unwrap();
        pager.flush().unwrap();

        let mut new_data = pager.read::<DataPage>(rid.page).unwrap();
        let new_buf = new_data.delete(rid.slot, &mut pager).unwrap();
        pager.write(new_data).unwrap();
        pager.flush().unwrap();

        assert_eq!(buf, new_buf);

        let new_new_data = pager.read::<DataPage>(rid.page).unwrap();
        assert_eq!(new_new_data.overflow, None);
        

    }
}
