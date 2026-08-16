use super::{Page, PageId, PageType, PageHeader, Slot, read_str, read_usize };

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

    fn deserialize(header: PageHeader, bytes: &mut &[u8]) -> StoreResult<Self> {
        let mut keys: Vec<String> = Vec::new();
        let mut children: Vec<PageId> = Vec::new();

        for _ in 0..header.slots {
            let slot = Slot::read(bytes)?;
            let mut slot_bytes = &bytes[slot.offset..slot.offset+slot.len];
            keys.push(read_str(&mut slot_bytes)?);
            children.push(PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!"));
        }

        Ok( Self { header, keys, children })
    }
}

