use super::{
    Page, PageId, PageType, PageHeader, Pager,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE, SLOT_POINTER_SIZE },
    read_str, read_usize, read_u16,
};

use crate::{
    errors::{StoreResult, StoreErr, TreeErr},
    store::{Rid, RID_SIZE},
};

pub struct LeafPage {
    header: PageHeader,
    pub keys: Vec<String>,
    pub rids: Vec<Rid>,
    pub next_leaf: Option<PageId>
}

impl LeafPage {
    pub fn new(id: PageId, keys: Vec<String>, rids: Vec<Rid>, next_leaf: Option<PageId>) -> Self {
        let slots = (keys.len() + 1) as u16;
        // One slot, 2 u16, 4 bytes total, then add 4 for next_leaf slot
        let lower = (keys.len() * SLOT_POINTER_SIZE + SLOT_POINTER_SIZE) as u16; 
        // A key/RID pair is keylen + usize + u16, which is 10, the 8 is for next_leaf slot
        let upper = (PAGE_SIZE - keys.iter().map(|k| k.len() + 10).sum::<usize>() - 8) as u16;

        Self {
            header: PageHeader{ id, pagetype: PageType::Leaf, next: None, slots, lower, upper },
            keys, rids, next_leaf
        }
    }

    pub fn split(&mut self, pager: &mut Pager) -> Self {
        let slot_mid = (PAGE_SIZE - self.header.upper as usize - 8) / 2;

        let mut num_bytes = 0;
        let mut slot_idx = 0;
        for (index, key) in self.keys.iter().enumerate() {
            num_bytes += key.len() + 10;
            if num_bytes >= slot_mid {
                slot_idx = index;
                break;
            }
        };

        let new_keys = self.keys.split_off(slot_idx);
        let new_rids = self.rids.split_off(slot_idx);
        let old_next = self.next_leaf;

        let new_id = pager.alloc();
        let new_page = LeafPage::new(new_id, new_keys, new_rids, old_next);

        self.next_leaf = Some(new_id);
        self.header.slots = (self.keys.len() + 1) as u16;
        self.header.lower = (self.keys.len() * SLOT_POINTER_SIZE + SLOT_POINTER_SIZE) as u16;
        self.header.upper = (PAGE_SIZE - self.keys
            .iter()
            .map(|k| k.len() + 10)
            .sum::<usize>() - 8) as u16;

        new_page
    }

    pub fn insert(&mut self, key: &str, rid: Rid) -> Option<Rid> {
        match self.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
            Ok(i) => {
                let old_rid = self.rids[i];
                self.rids[i] = rid;
                Some(old_rid)
            },
            Err(i) => {
                self.header.slots += 1;
                self.header.lower += SLOT_POINTER_SIZE as u16;
                self.header.upper -= (RID_SIZE + key.len()) as u16;

                self.rids.insert(i, rid);
                self.keys.insert(i, key.to_string());
                None
            },
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

    // Slot is usize for page, u16 for slot, and rest for key string.
    // Last slot is usize for next_leaf.
    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for (i, key) in self.keys.iter().enumerate() {
            let rid = self.rids.get(i)
                .ok_or(StoreErr::TreeErr(TreeErr::KeyCountErr(self.header.id)))?;

            let mut body: Vec<u8> = Vec::new();
            body.extend_from_slice(&rid.page.get().to_le_bytes());
            body.extend_from_slice(&(rid.slot as u16).to_le_bytes());
            body.extend_from_slice(key.as_bytes());

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
            let page = PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!");
            let slot = read_u16(&mut slot_bytes)?;
            rids.push(Rid { page, slot });

            keys.push(read_str(&mut slot_bytes)?);
        }

        let next_leaf = PageId::new(read_usize(&mut cursor.next()?)?);

        Ok(Self { header, keys, rids, next_leaf })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn leaf_serialize() {
    }

    #[test]
    fn leaf_deserialize() {
    }

    #[test]
    fn leaf_round_trip() {
    }

    #[test]
    fn leaf_split() {
    }

    #[test]
    fn leaf_insert() {
    }
}
