use super::{
    Page, PageId, PageType, PageHeader,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE },
    read_usize,
};
use crate::errors::{StoreResult, StoreErr};

pub struct OverflowPage {
    header: PageHeader,
    next: Option<PageId>,
    data: Vec<u8>,
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

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].clone_from_slice(&self.header.serialize());

        let len = PAGE_SIZE - 8 - self.data.len();
        let dir = PAGEHEADER_SIZE;
        if len < dir + 8 {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: 8 - self.data.len(),
                pagetype: PageType::Data
            });
        }

        let next = self.next
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[PAGE_SIZE-8..PAGE_SIZE].copy_from_slice(&next);
        bytes[len..PAGE_SIZE-8].copy_from_slice(&self.data);

        bytes[dir..dir+2].copy_from_slice(&((PAGE_SIZE - 8) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(8 as u16).to_le_bytes());
        bytes[dir+4..dir+6].copy_from_slice(&((PAGE_SIZE - 8 - len) as u16).to_le_bytes());
        bytes[dir+6..dir+8].copy_from_slice(&(len as u16).to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let next = PageId::new(read_usize(&mut cursor.next()?)?);
        let data = cursor.next()?.to_vec();

        Ok(Self { header, next, data })
    }
}

