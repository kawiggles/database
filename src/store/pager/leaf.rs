use super::{
    Page, PageId, PageType, PageHeader, 
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE },
    read_str, read_usize
};

use crate::{
    errors::{StoreResult, StoreErr, TreeErr},
    store::Rid,
};

pub struct LeafPage {
    header: PageHeader,
    pub keys: Vec<String>,
    pub rids: Vec<Rid>,
    next_leaf: Option<PageId>
}

impl LeafPage {
    pub fn new(id: PageId, key: String, rid: Rid, next_leaf: Option<PageId>) -> Self {
        Self {
            header: PageHeader { 
                id,
                pagetype: PageType::Leaf,
                next: None,
                slots: 1,
                lower: 4, // One slot, 2 u16, 4 bytes total
                // 1 key/RID pair, which is u16 + keylen + usize + u16
                upper: (PAGE_SIZE - (12 + key.len())) as u16, 
            },
            keys: vec![key],
            rids: vec![rid],
            next_leaf,
        }
    }
}

impl Page for LeafPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype() -> PageType {
        PageType::Leaf
    }

    fn free_space(&self) -> usize {
        (self.header.upper - self.header.lower) as usize
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for (i, key) in self.keys.iter().enumerate() {
            let rid = self.rids.get(i)
                .ok_or(StoreErr::TreeErr(TreeErr::KeyCountErr(self.header.id)))?;

            let mut body: Vec<u8> = Vec::new();
            body.extend_from_slice(&(key.len() as u16).to_le_bytes());
            body.extend_from_slice(key.as_bytes());
            body.extend_from_slice(&rid.page.get().to_le_bytes());
            body.extend_from_slice(&(rid.slot as u16).to_le_bytes());

            let len = body.len();
            let offset = end - len;

            if offset < dir + 4 {
                return Err(StoreErr::SlotOverwrite {
                    page: self.header.id,
                    len,
                    pagetype: PageType::Leaf,
                });
            }

            bytes[offset..end].copy_from_slice(&body);
            end = offset;

            bytes[dir..dir+2].copy_from_slice(&(offset as u16).to_le_bytes());
            bytes[dir+2..dir+4].copy_from_slice(&(len as u16).to_le_bytes());
            dir += 4;
        }

        if end - 8 < dir + 4 {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: 8,
                pagetype: PageType::Leaf,
            });
        }

        let next = self.next_leaf
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[end-8..end].copy_from_slice(&next);
        bytes[dir..dir+2].copy_from_slice(&((end - 8) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(8 as u16).to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut keys: Vec<String> = Vec::new();
        let mut rids: Vec<Rid> = Vec::new();
        
        for _ in 0..header.slots - 1 {
            let mut slot_bytes = cursor.next()?;
            keys.push(read_str(&mut slot_bytes)?);
            let page = PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!");
            let slot = read_usize(&mut slot_bytes)?;
            rids.push(Rid { page, slot });
        }

        let next_leaf = PageId::new(read_usize(&mut cursor.next()?)?);

        Ok(Self { header, keys, rids, next_leaf })
    }
}
