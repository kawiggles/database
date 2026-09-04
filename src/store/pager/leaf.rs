use super::{
    Page, PageId, PageType, PageHeader, Pager,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE, SLOT_POINTER_SIZE, PAGEID_SIZE},
    read_str, read_usize, read_u16,
};

use crate::{
    errors::{StoreErr, StoreResult, TreeErr, UserResult, UserErr},
    store::{RID_SIZE, Rid},
};

#[derive(Debug, PartialEq)]
pub struct LeafPage {
    header: PageHeader,
    pub keys: Vec<String>,
    pub rids: Vec<Rid>,
    pub next_leaf: Option<PageId>
}

impl LeafPage {
    pub fn new(id: PageId, keys: Vec<String>, rids: Vec<Rid>, next_leaf: Option<PageId>) -> Self {
        let slots = (keys.len() + 1) as u16;
        let lower = PAGEHEADER_SIZE as u16 + slots * SLOT_POINTER_SIZE as u16; 
        let upper = (PAGE_SIZE - keys
            .iter()
            .map(|k| k.len() + RID_SIZE)
            .sum::<usize>() - PAGEID_SIZE) as u16;

        Self {
            header: PageHeader{ id, pagetype: PageType::Leaf, next: None, slots, lower, upper },
            keys, rids, next_leaf
        }
    }

    pub fn split(&mut self, pager: &mut Pager) -> Self {
        let slot_mid = (PAGE_SIZE - self.header.upper as usize - PAGEID_SIZE) / 2;

        let mut num_bytes = 0;
        let mut slot_idx = 0;
        for (index, key) in self.keys.iter().enumerate() {
            num_bytes += key.len() + RID_SIZE;
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
        self.header.lower = PAGEHEADER_SIZE as u16 + self.header.slots * SLOT_POINTER_SIZE as u16;
        self.header.upper = (PAGE_SIZE - self.keys
            .iter()
            .map(|k| k.len() + RID_SIZE)
            .sum::<usize>() - PAGEID_SIZE) as u16;

        debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
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

                debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
                None
            },
        }
    }

    pub fn delete(&mut self, key: &str) -> UserResult<Rid> {
        match self.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
            Ok(i) => {
                self.header.slots -= 1;
                self.header.lower -= SLOT_POINTER_SIZE as u16;
                self.header.upper += (RID_SIZE + key.len()) as u16;

                self.keys.remove(i);

                debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
                Ok(self.rids.remove(i))
            },
            Err(_) => Err(UserErr::NoRID(key.into())),
        }
    }

    pub fn borrow_from(&mut self, sibling: &mut Self, from_left: bool) -> String {
        let (key, rid) = if from_left {
            (sibling.keys.pop().expect("Leaf with no keys found!"),
            sibling.rids.pop().expect("Leaf with no RIDs found!"))
        } else {
            sibling.keys.remove(0);
            (sibling.keys[0].clone(), sibling.rids.remove(0))
        };

        if from_left {
            self.keys.insert(0, key.clone());
            self.rids.insert(0, rid);
        } else {
            self.keys.push(key.clone());
            self.rids.push(rid);
        }

        self.refresh_header();
        sibling.refresh_header();

        debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
        key
    }

    pub fn merge(&mut self, other: Self) {
        self.keys.extend(other.keys);
        self.rids.extend(other.rids);
        self.next_leaf = other.next_leaf;

        self.header.slots = self.keys.len() as u16 + 1;
        self.header.lower = PAGEHEADER_SIZE as u16 + self.header.slots * SLOT_POINTER_SIZE as u16;
        self.header.upper = (PAGE_SIZE - self.keys.iter()
            .map(|k| RID_SIZE + k.len())
            .sum::<usize>() - PAGEID_SIZE) as u16;

        debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
    }

    pub fn refresh_header(&mut self) {
        self.header.slots = self.keys.len() as u16 + 1;
        self.header.lower = PAGEHEADER_SIZE as u16 + self.header.slots * SLOT_POINTER_SIZE as u16;
        self.header.upper = (PAGE_SIZE - self.keys.iter()
            .map(|k| RID_SIZE + k.len())
            .sum::<usize>() - PAGEID_SIZE) as u16;

        debug_assert_eq!(self.keys.len() + 1, self.header.slots as usize);
    }
}

impl Page for LeafPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn pagetype() -> PageType {
        PageType::Leaf
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        debug_assert_eq!(self.header.slots as usize, self.keys.len() + 1);

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

            if offset < dir + SLOT_POINTER_SIZE {
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
            dir += SLOT_POINTER_SIZE;
        }

        if end - PAGEID_SIZE < dir + SLOT_POINTER_SIZE {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: PAGEID_SIZE,
                pagetype: PageType::Leaf,
            });
        }

        let next = self.next_leaf
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[end-PAGEID_SIZE..end].copy_from_slice(&next);
        bytes[dir..dir+2].copy_from_slice(&((end - PAGEID_SIZE) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(PAGEID_SIZE as u16).to_le_bytes());

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
    use super::*;

    #[test]
    fn serialize_and_deserialize() {
        let leaf = LeafPage::new(PageId::new(1).unwrap(),
            vec!["astring".into()], 
            vec![Rid{ page: PageId::new(3).unwrap(), slot: 1 }],
            None);
        println!("{:?}", leaf);
        let bytes = leaf.serialize().unwrap();
        println!("{}", bytes.len());
        let mut cursor = PageCursor::new(&bytes.as_slice());

        let header = leaf.header.clone();
        println!("{:?}", leaf.header());
        let new_leaf = LeafPage::deserialize(header, &mut cursor).unwrap();
        println!("{:?}", new_leaf);

        assert_eq!(leaf, new_leaf);
    }

    #[test]
    fn insert() {
        let mut leaf = LeafPage::new(PageId::new(1).unwrap(),
            vec!["akey".into()], 
            vec![Rid{ page: PageId::new(3).unwrap(), slot: 1 }],
            None);

        leaf.insert("anotherkey", Rid { page: PageId::new(4).unwrap(), slot: 2 });

        let new_leaf = LeafPage::new(PageId::new(1).unwrap(),
            vec!["akey".into(), "anotherkey".into()], 
            vec![Rid{ page: PageId::new(3).unwrap(), slot: 1 },
                Rid{ page: PageId::new(4).unwrap(), slot: 2 }],
            None);

        assert_eq!(leaf, new_leaf);
    }
}
