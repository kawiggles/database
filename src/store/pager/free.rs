use super::{
    Page, PageHeader, PageId, PageType,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE, SLOT_POINTER_SIZE, PAGEID_SIZE},
    read_usize,
};
use crate::errors::StoreResult;

#[derive(Debug, PartialEq)]
pub struct FreePage {
    pub header: PageHeader,
    pub next: Option<PageId>,
}

impl FreePage {
    pub fn new(id: PageId) -> Self {
        Self {
            header: PageHeader {
                id,
                pagetype: PageType::Free,
                slots: 1,
                lower: (PAGEHEADER_SIZE + SLOT_POINTER_SIZE) as u16,
                upper: (PAGE_SIZE - PAGEID_SIZE) as u16,
            },
            next: None,
        }
    }

    pub fn append(&mut self, next: PageId) {
        self.next = Some(next);
    }

    pub fn sever(&mut self) {
        self.next = None;
    }
}

impl Page for FreePage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn set_id(&mut self, id: PageId) {
        self.header.id = id;
    }

    fn pagetype() -> PageType {
        PageType::Free
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let next = self.next
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[PAGEHEADER_SIZE..PAGEHEADER_SIZE+2]
            .copy_from_slice(&((PAGE_SIZE - 8) as u16).to_le_bytes());
        bytes[PAGEHEADER_SIZE+2..PAGEHEADER_SIZE+4]
            .copy_from_slice(&(8 as u16).to_le_bytes());

        bytes[PAGE_SIZE-8..PAGE_SIZE].copy_from_slice(&next);

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let next = PageId::new(read_usize(&mut cursor.next()?)?);
        Ok(Self { header, next })
    }
}
