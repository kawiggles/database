use super::{Page, PageId, PageType, PageHeader};

use crate::errors::StoreResult;
use crate::store::value::Value;

pub struct DataPage {
    header: PageHeader,
    overflow: Option<PageId>,
}

impl DataPage {
    pub fn new() -> Self {
        todo!()
    }

    pub fn get_slot(&mut self, slot: usize) -> StoreResult<Value> {
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

