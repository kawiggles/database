use super::{
    Page, PageId, PageType, PageHeader, 
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE},
    read_str, read_usize
};

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
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes.extend_from_slice(&self.header.serialize());

        let mut start = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;
        for (i, key) in self.keys.iter().enumerate() {
            let child = self.children.get(i).expect("Error, key/child mismatch in a branch!");
            let mut body: Vec<u8> = Vec::new();
            body.extend_from_slice(&key.len().to_le_bytes());
            body.extend_from_slice(key.as_bytes());
            body.extend_from_slice(&child.get().to_le_bytes());

            bytes.get_mut(/*start and end..*/)
        }

        bytes
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut keys: Vec<String> = Vec::new();
        let mut children: Vec<PageId> = Vec::new();

        for _ in 0..header.slots {
            let mut slot_bytes = cursor.next()?
            keys.push(read_str(&mut slot_bytes)?);
            children.push(PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!"));
        }

        Ok( Self { header, keys, children })
    }
}

