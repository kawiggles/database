use std::iter::Rev;

use super::{
    Page, PageId, PageType, PageHeader, Pager,
    page::{PageCursor, PAGE_SIZE, PAGEHEADER_SIZE, SLOT_POINTER_SIZE, PAGEID_SIZE},
    read_str, read_usize
};

use crate::errors::{StoreResult, StoreErr, TreeErr};
use crate::store::BpTree;

#[derive(Debug, PartialEq)]
pub struct BranchPage {
    pub header: PageHeader,
    pub keys: Vec<String>,
    pub children: Vec<PageId>,
}

impl BranchPage {
    pub fn new(id: PageId, keys: Vec<String>, children: Vec<PageId>) -> Self {
        let slots = children.len() as u16;
        let lower = (PAGEHEADER_SIZE + children.len() * SLOT_POINTER_SIZE) as u16;
        let upper = (PAGE_SIZE - keys
            .iter()
            .map(|k| k.len() + PAGEID_SIZE)
            .sum::<usize>() - PAGEID_SIZE) as u16;

        Self {
            header: PageHeader { id, pagetype: PageType::Branch, next: None, slots, lower, upper },
            keys, children
        }
    }
    
    pub fn insert(&mut self, key: String, id: PageId) {
        let i = self.keys
            .binary_search_by(|probe| probe.as_str().cmp(&key))
            .unwrap_or_else(|i| i);

        self.keys.insert(i, key);
        self.children.insert(i + 1, id);
        self.refresh_header();
    }

    pub fn split<'a, I>(&mut self, pager: &mut Pager, path: &mut Rev<I>, tree: &mut BpTree)
        -> StoreResult<()> where I: Iterator<Item = &'a PageId> + DoubleEndedIterator {
        let slot_mid = (PAGE_SIZE - self.header.upper as usize - PAGEID_SIZE) / 2;

        let mut num_bytes = 0;
        let mut slot_idx = 0;
        for (index, key) in self.keys.iter().enumerate() {
            num_bytes += key.len() + PAGEID_SIZE;
            if num_bytes >= slot_mid {
                slot_idx = index;
                break;
            }
        };

        let new_keys = self.keys.split_off(slot_idx);
        let new_children = self.children.split_off(slot_idx);

        let new_id = pager.alloc();
        let new_page = BranchPage::new(new_id, new_keys, new_children);
        let promoted = self.keys.pop().expect("Branch with no keys found!");

        self.refresh_header();
        debug_assert_eq!(self.header.slots as usize, self.children.len());
        debug_assert_eq!(new_page.header.slots as usize, new_page.children.len());

        pager.write(new_page)?;

        if let Some(&parent_id) = path.next() {
            let mut parent = pager.read::<BranchPage>(parent_id)?;
            parent.insert(promoted, new_id);
            if parent.free_space() == None {
                parent.split(pager, path, tree)?;
            }
            pager.write(parent)?;
        } else {
            let root_id = pager.alloc();
            let parent = BranchPage::new(root_id, vec![promoted], vec![self.header.id, new_id]);
            tree.root = Some(root_id);
            pager.write(parent)?;
        }

        Ok(())
    }

    pub fn borrow_from(&mut self, sibling: &mut Self, from_left: bool, old_sep: String) -> String {
        let (key, child) = if from_left {
            (sibling.keys.pop().expect("Branch with no keys found!"),
            sibling.children.pop().expect("Branch with no children found!"))
        } else {
            (sibling.keys.remove(0), sibling.children.remove(0))
        };

        if from_left {
            self.keys.insert(0, old_sep);
            self.children.insert(0, child);
        } else {
            self.keys.push(old_sep);
            self.children.push(child);
        }

        self.refresh_header();
        sibling.refresh_header();

        debug_assert_eq!(self.header.slots as usize, self.children.len());
        key
    }

    pub fn merge(&mut self, other: Self) {
        self.keys.extend(other.keys);
        self.children.extend(other.children);
        
        self.refresh_header();

        debug_assert_eq!(self.header.slots as usize, self.children.len());
    }

    pub fn refresh_header(&mut self) {
        self.header.slots = self.children.len() as u16;
        self.header.lower = PAGEHEADER_SIZE as u16 + self.header.slots * SLOT_POINTER_SIZE as u16;

        let sum = self.keys.iter().map(|k| PAGEID_SIZE + k.len()).sum::<usize>();
        self.header.upper = PAGE_SIZE
            .checked_sub(sum)
            .and_then(|v| v.checked_sub(PAGEID_SIZE))
            .unwrap_or(0) as u16;

        debug_assert_eq!(self.header.slots as usize, self.children.len());
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
        debug_assert_eq!(self.header.slots as usize, self.children.len());

        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].copy_from_slice(&self.header.serialize());

        let mut dir = PAGEHEADER_SIZE;
        let mut end = PAGE_SIZE;

        for (i, key) in self.keys.iter().enumerate() {
            let child = self.children.get(i)
                .ok_or(StoreErr::TreeErr(TreeErr::KeyCountErr(self.header.id)))?;
            let mut body: Vec<u8> = Vec::new();
            body.extend_from_slice(&child.get().to_le_bytes());
            body.extend_from_slice(key.as_bytes());

            let len = body.len();
            let offset = end - len;

            if offset < dir + SLOT_POINTER_SIZE {
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
            dir += SLOT_POINTER_SIZE;
        }

        let last_child = self.children.last()
            .ok_or(StoreErr::TreeErr(TreeErr::KeyChildDesync(self.header.id)))?
            .get()
            .to_le_bytes();

        if end - PAGEID_SIZE < dir + SLOT_POINTER_SIZE {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: PAGEID_SIZE,
                pagetype: PageType::Branch,
            });
        }

        bytes[end-PAGEID_SIZE..end].copy_from_slice(&last_child);
        bytes[dir..dir+2].copy_from_slice(&((end - PAGEID_SIZE) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(8 as u16).to_le_bytes());

        // TODO: check that dir and end match lower and upper
        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let mut keys: Vec<String> = Vec::new();
        let mut children: Vec<PageId> = Vec::new();

        if header.slots == 0 {
            return Err(StoreErr::EmptyPage { page: header.id, pagetype: header.pagetype });
        }

        for _ in 0..header.slots - 1 {
            let mut slot_bytes = cursor.next()?;
            children.push(PageId::new(read_usize(&mut slot_bytes)?)
                .expect("Attempted to read PageId 0!!!"));
            keys.push(read_str(&mut slot_bytes)?);
        }

        children.push(PageId::new(read_usize(&mut cursor.next()?)?)
                .expect("Attempted to read PageId 0!!!"));

        Ok(Self { header, keys, children })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_and_deserialize() {
        let branch = BranchPage::new(
            PageId::new(1).unwrap(),
            vec!["akey".into()],
            vec![PageId::new(2).unwrap(), PageId::new(3).unwrap()],
        );

        let bytes = branch.serialize().unwrap();
        let mut cursor = PageCursor::new(&bytes);
        let header = branch.header.clone();
        let new_branch = BranchPage::deserialize(header, &mut cursor).unwrap();

        assert_eq!(branch, new_branch);
    }
}
