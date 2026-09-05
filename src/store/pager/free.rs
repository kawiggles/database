use super::{
    Page, PageHeader, PageId, PageType,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE},
};
use crate::errors::StoreResult;

pub struct FreePage(pub PageHeader);

impl FreePage {
    pub fn new(id: PageId) -> Self {
        Self(PageHeader {
            id,
            pagetype: PageType::Free,
            next: None,
            slots: 0,
            lower: 0,
            upper: 0
        })
    }
}

impl FreePage {
    pub fn sever(&mut self) {
        self.0.next = None;
    }
}

impl Page for FreePage {
    fn header(&self) -> &PageHeader {
        &self.0
    }

    fn pagetype() -> PageType {
        PageType::Free
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.0.serialize());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, _cursor: &mut PageCursor) -> StoreResult<Self> {
        Ok(Self(header))
    }
}
