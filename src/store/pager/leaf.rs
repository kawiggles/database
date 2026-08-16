use super::{Page, PageId, PageType, PageHeader};

use crate::{
    errors::StoreResult,
    store::RID,
};

pub struct LeafPage {
    header: PageHeader,
    pub keys: Vec<String>,
    pub rids: Vec<RID>,
    next_leaf: Option<PageId>
}

impl LeafPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for LeafPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype() -> PageType {
        PageType::Leaf
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> StoreResult<Self> {
        todo!()
    }
}

