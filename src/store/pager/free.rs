use super::{Page, PageHeader, PageId, PageType, Oid, };
use crate::errors::StoreResult;

pub struct FreePage(pub PageHeader);

impl FreePage {
    pub fn new(id: PageId) -> Self {
        Self(PageHeader {
            id,
            table_oid: Oid(0),
            pagetype: PageType::Free,
            next: None,
            slots: 0,
            lower: 0,
            upper: 0
        })
    }
}

impl Page for FreePage {
    fn header(&self) -> &PageHeader {
        &self.0
    }

    fn pagetype() -> PageType {
        PageType::Free
    }

    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> StoreResult<Self> {
        todo!()
    }
}
