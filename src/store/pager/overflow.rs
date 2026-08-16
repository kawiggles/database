use super::{Page, PageId, PageType, PageHeader};
use crate::errors::StoreResult;

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

    fn deserialize(bytes: &[u8]) -> StoreResult<Self> {
        todo!()
    }
}

