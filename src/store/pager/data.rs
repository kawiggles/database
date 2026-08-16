use super::{Page, PageId, PageType, PageHeader};

use crate::errors::StoreResult;

pub struct DataPage {
    header: PageHeader,
    overflow: Option<PageId>,
}

impl DataPage {
    pub fn new() -> Self {
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

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> StoreResult<Self> {
        todo!()
    }
}

