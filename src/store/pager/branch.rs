use super::{
    Page, PageId, PageType, PageHeader, 
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE},
    read_str, read_usize
};

use crate::errors::{StoreResult, StoreErr, TreeErr};

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

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for (i, key) in self.keys.iter().enumerate() {
            let child = self.children.get(i)
                .ok_or(StoreErr::TreeErr(TreeErr::KeyCountErr(self.header.id)))?;
            let mut body: Vec<u8> = Vec::new();
            body.extend_from_slice(&(key.len() as u16).to_le_bytes());
            body.extend_from_slice(key.as_bytes());
            body.extend_from_slice(&child.get().to_le_bytes());

            let len = body.len();
            let offset = end - len;

            if offset < dir + 4 {
                return Err(StoreErr::SlotOverwrite {
                    page: self.header.id,
                    len,
                    pagetype: PageType::Branch,
                });
            }

            bytes[offset..end].copy_from_slice(&body);
            end = offset;

            bytes[dir..dir+2].copy_from_slice(&(offset as u16).to_le_bytes());
            bytes[dir+2..dir+4].copy_from_slice(&(len as u16).to_le_bytes());
            dir += 4;
        }

        let last_child = self.children.last()
            .ok_or(StoreErr::TreeErr(TreeErr::KeyChildDesync(self.header.id)))?
            .get()
            .to_le_bytes();

        if end - 8 < dir + 4 {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: 8,
                pagetype: PageType::Branch,
            });
        }

        bytes[end-8..end].copy_from_slice(&last_child);
        bytes[dir..dir+2].copy_from_slice(&((end - 8) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(8 as u16).to_le_bytes());

        // TODO: check that dir and end match lower and upper
        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut keys: Vec<String> = Vec::new();
        let mut children: Vec<PageId> = Vec::new();

        for _ in 0..header.slots - 1 {
            let mut slot_bytes = cursor.next()?;
            keys.push(read_str(&mut slot_bytes)?);
            children.push(PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!"));
        }

        children.push(PageId::new(read_usize(&mut cursor.next()?)?)
                .expect("Attempted to read PageId 0!!!"));

        Ok(Self { header, keys, children })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn branch_serialize() {
    }

    #[test]
    fn branch_deserialize() {
    }

    #[test]
    fn branch_round_trip() {
    }
}
