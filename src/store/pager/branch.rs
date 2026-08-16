use super::{Page, PageId, PageType, PageHeader};

use crate::errors::StoreResult;

pub struct BranchPage {
    pub header: PageHeader,
    pub keys: Vec<String>,
    pub children: Vec<PageId>,
}

impl BranchPage {
    pub fn new() -> Self {
        todo!()
    }
}

impl Page for BranchPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype() -> PageType {
        PageType::Branch
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> StoreResult<Self> {
        todo!();
        /*
        let header = ;
        let keys = ;
        let children = ;

        Ok( Self { header, keys, children })
        */
    }
}

