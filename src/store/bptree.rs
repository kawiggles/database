use crate::{
    errors::{DbResult, StoreErr, UserErr, StoreResult, TreeErr},
    store::{
        Rid,
        pager::{AnyPage, LeafPage, BranchPage, Page, PageId, PageType, Pager, page::PAGE_CAPACITY},
    }
};

pub struct BpTree {
    pub root: Option<PageId>,
}

impl BpTree {
    // Easiest method on the tree
    pub fn new(root: Option<PageId>) -> Self {
        BpTree { root }
    }

    pub fn get(&self, key: &str, pager: &mut Pager) -> DbResult<Rid> {
        let mut current = match self.root {
            Some(x) => x,
            None => return Err(UserErr::NoRoot)?,
        };

        loop {
            let page = pager.read_any(current)?;
            match page {
                AnyPage::Branch(branch) => {
                    let i = match branch.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => i + 1, // On a hit, go to the right
                        Err(i) => i, // A miss is where we would go if it existed
                    };
                    current = branch.children[i];
                },
                AnyPage::Leaf(leaf) => { 
                    let index = leaf.keys
                        .binary_search_by(|probe| { probe.as_str().cmp(key) })
                        .map_err(|_| UserErr::NoRID(key.into()))?;

                    return Ok(leaf.rids[index]);
                },
                _ => { 
                    return Err(StoreErr::UnexpectedPagetype{
                        found: page.to_pagetype(),
                        expected: PageType::Branch,
                    })?;
                }
            }
        }
    }

    // Returns Some(Rid) if the associated RID needs to be deleted
    pub fn insert(&mut self, key: &str, rid: Rid, pager: &mut Pager) -> DbResult<Option<Rid>> {
        // If the tree is empty, create a new root
        let Some(root) = self.root else {
            let new_id = pager.alloc();
            let page = LeafPage::new(new_id, vec![key.to_string()], vec![rid], None);
            pager.write(page)?;
            pager.flush()?;

            self.root = Some(new_id);
            return Ok(None);
        };

        // First: find the leaf page while tracking path
        let mut path: Vec<PageId> = Vec::new();
        let mut current = root;
        loop {
            let page = pager.read_any(current)?;
            match page {
                AnyPage::Branch(branch) => {
                    path.push(current);
                    let i = match branch.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => i + 1,
                        Err(i) => i,
                    };
                    current = branch.children[i];
                },
                AnyPage::Leaf(_) => break,
                _ => { 
                    return Err(StoreErr::UnexpectedPagetype{
                        found: page.to_pagetype(),
                        expected: PageType::Branch,
                    })?;
                }
            }
        }
        let mut path = path.iter().rev().peekable();

        // Second: insert key and rid into leaf
        let mut page = pager.read::<LeafPage>(current)?;
        let replaced = page.insert(key, rid);

        // Third: split the leaf if needed
        if page.free_space() == None {
            let new_page = page.split(pager);
            let new_id = new_page.header().id;

            let promoted = new_page.keys[0].clone();
            pager.write(new_page)?;
            pager.write(page)?;

            // Then we promote the key to the parent branch
            if let Some(&parent_id) = path.peek() {
                let mut parent = pager.read::<BranchPage>(*parent_id)?;
                let i = parent.keys
                    .binary_search_by(|probe| probe.as_str().cmp(&promoted))
                    .unwrap_or_else(|i| i);
                parent.keys.insert(i, promoted);
                debug_assert!(parent.children[i] == current);
                parent.children.insert(i + 1, new_id);
                pager.write(parent)?;
            } else {
                // If there's no parent, we make a new root
                let root_id = pager.alloc();
                let parent = BranchPage::new(root_id, vec![promoted], vec![current, new_id]);
                self.root = Some(root_id);
                pager.write(parent)?;
            }
        } else {
            pager.write(page)?;
        }

        // Fourth: repeat the above, but this time all for branch pages, iterating through the path
        while let Some(id) = path.next() {
            let mut page = pager.read::<BranchPage>(*id)?;
            if page.free_space() == None {
                let (promoted, new_page) = page.split(pager);
                let new_id = new_page.header().id;

                pager.write(page)?;
                pager.write(new_page)?;

                // Tried writing this as function because copied code, but the signature was insane
                if let Some(&parent_id) = path.peek() {
                    let mut parent = pager.read::<BranchPage>(*parent_id)?;
                    let i = parent.keys
                        .binary_search_by(|probe| probe.as_str().cmp(&promoted))
                        .unwrap_or_else(|i| i);
                    parent.keys.insert(i, promoted);
                    debug_assert!(parent.children[i] == *id);
                    parent.children.insert(i + 1, new_id);
                    pager.write(parent)?;
                } else {
                    let root_id = pager.alloc();
                    let parent = BranchPage::new(root_id, vec![promoted], vec![*id, new_id]);
                    self.root = Some(root_id);
                    pager.write(parent)?;
                }
            } 
        }

        pager.flush()?;
        Ok(replaced)
    }

    // Holy fucking shit (Tool reference)
    // No fucking kidding, past me. WTF is this???
    pub fn remove(&mut self, key: &str, pager: &mut Pager) -> DbResult<Rid> {
        // Handle empty tree case
        let Some(root) = self.root else {
            return Err(UserErr::NoRoot)?;
        };

        // First: search for the leaf node with the key to delete
        let mut path: Vec<PageId> = Vec::new();
        let mut current = root;
        loop {
            let page = pager.read_any(current)?;
            match page {
                AnyPage::Branch(branch) => {
                    path.push(current);
                    let i = match branch.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => i + 1,
                        Err(i) => i,
                    };
                    current = branch.children[i];
                }
                AnyPage::Leaf(_) => break,
                _ => {
                    return Err(StoreErr::UnexpectedPagetype{
                        found: page.to_pagetype(),
                        expected: PageType::Branch,
                    })?;
                },
            }
        }
        let mut path = path.iter().rev().peekable();

        // Second: delete the key and rid
        let mut leaf = pager.read::<LeafPage>(current)?;
        let removed = leaf.delete(key)?;

        // Third: handle leaf underflow if needed
        if leaf.free_space().unwrap() >= PAGE_CAPACITY / 2 {
            if let Some(&&parent_id) = path.peek() {
                let mut parent = pager.read::<BranchPage>(parent_id)?;

                // Collect siblings from the parent
                let pos = parent.children.iter().position(|&c| c == leaf.header().id)
                    .expect("Leaf not found in own parent branch");
                let siblings = [
                    (pos > 0).then(|| (parent.children[pos-1], true)),
                    (pos < parent.children.len() - 1).then(|| (parent.children[pos+1], false))
                ];

                // Do a borrow from a sibling if we can
                let mut borrowed = false;
                for (sib_id, is_left) in siblings.into_iter().flatten() {
                    let mut sibling = pager.read::<LeafPage>(sib_id)?;
                    if sibling.free_space().unwrap() < PAGE_CAPACITY / 2 {
                        let borrowed_key = leaf.borrow_from(&mut sibling, is_left);
                        if is_left {
                            parent.keys[pos-1] = borrowed_key;
                        } else {
                            parent.keys[pos] = borrowed_key;
                        }

                        pager.write(sibling)?;
                        borrowed = true;
                        break;
                    }
                }

                // If we did borrow, we can write and flush, if not, we gotta do a merge/loop
                if borrowed {
                    pager.write(leaf)?;
                    pager.flush()?;
                    return Ok(removed);
                } else { // Merge logic, boy howdy
                    if pos < parent.children.len() - 1 { // right sibling first, better complexity
                        let sib_id = parent.children[pos+1];
                        let sibling = pager.read::<LeafPage>(sib_id)?;
                        leaf.merge(sibling);
                        pager.free(sib_id)?;

                        parent.keys.remove(pos);
                        parent.children.remove(pos + 1);
                        pager.write(parent)?;

                        pager.write(leaf)?;
                    } else if pos > 0 { // left sibling
                        let sib_id = parent.children[pos-1];
                        let mut sibling = pager.read::<LeafPage>(sib_id)?;
                        sibling.merge(leaf);
                        pager.free(current)?;

                        parent.keys.remove(pos - 1);
                        parent.children.remove(pos);
                        pager.write(parent)?;

                        pager.write(sibling)?;
                    } else {
                        unreachable!("How tf did you get a leaf with no siblings and a parent???");
                    }
                }
            } else { // Means that the leaf is the root
                if leaf.keys.is_empty() { // So if the delete emptied it, free it 
                    self.root = None;
                    pager.free(current)?;
                } else {
                    pager.write(leaf)?;
                }
            }
        } else {
            pager.write(leaf)?;
            pager.flush()?;
            return Ok(removed);
        }

        // Fourth: loop through the path doing this until we stop merging or we get to the root
        while let Some(id) = path.next() {
            let mut page = pager.read::<BranchPage>(*id)?;
            if page.free_space().unwrap() >= PAGE_CAPACITY / 2 {
                if let Some(&&parent_idx) = path.peek() {
                    let mut parent = pager.read::<BranchPage>(parent_idx)?;

                    let pos = parent.children.iter().position(|&c| c == page.header().id)
                        .expect("Branch not found in own parent branch");
                    let siblings = [
                        (pos > 0).then(|| (parent.children[pos-1], true)),
                        (pos < parent.children.len() - 1).then(|| (parent.children[pos+1], false))
                    ];

                    // Do a borrow from a sibling if we can
                    let mut borrowed = false;
                    for (sib_id, is_left) in siblings.into_iter().flatten() {
                        let mut sibling = pager.read::<BranchPage>(sib_id)?;
                        if sibling.free_space().unwrap() < PAGE_CAPACITY / 2 {
                            let borrowed_key = page.borrow_from(&mut sibling, is_left);
                            if is_left {
                                parent.keys[pos-1] = borrowed_key;
                            } else {
                                parent.keys[pos] = borrowed_key;
                            }

                            pager.write(sibling)?;
                            borrowed = true;
                            break;
                        }
                    }

                    // If we did borrow, we can write and flush, if not, we gotta do a merge/loop
                    if borrowed {
                        pager.write(page)?;
                        pager.flush()?;
                        break;
                    } else { // Merge logic, boy howdy
                        if pos < parent.children.len() - 1 { // right sibling first, better complexity
                            let sib_id = parent.children[pos+1];
                            let sibling = pager.read::<BranchPage>(sib_id)?;
                            page.merge(sibling);
                            pager.free(sib_id)?;

                            let sep_key = parent.keys.remove(pos);
                            parent.children.remove(pos + 1);
                            pager.write(parent)?;

                            page.keys.push(sep_key);
                            pager.write(page)?;
                        } else if pos > 0 { // left sibling
                            let sib_id = parent.children[pos-1];
                            let mut sibling = pager.read::<BranchPage>(sib_id)?;
                            sibling.merge(page);
                            pager.free(current)?;

                            let sep_key = parent.keys.remove(pos - 1);
                            parent.children.remove(pos);
                            pager.write(parent)?;

                            sibling.keys.push(sep_key);
                            pager.write(sibling)?;
                        } else {
                            unreachable!("How tf did you get a branch with no siblings and a parent???");
                        }
                    }
                } else { break; }
            } else { break; }
        }

        let root_page = pager.read::<BranchPage>(self.root.unwrap())?;
        if root_page.keys.is_empty() {
            self.root = Some(root_page.children[0]);

        }

        pager.flush()?;
        Ok(removed)
    }

    pub fn validate(&self, pager: &mut Pager) -> StoreResult<()> {
        let Some(root) = self.root else {
            return Err(TreeErr::Empty)?;
        };

        let header = pager.read_header(root)?;
        if header.pagetype == PageType::Branch {
            let page = pager.read::<BranchPage>(root)?;
            if page.children.len() < 2 {
                return Err(TreeErr::RootTooFewChildren)?;
            }
        }

        let mut leaf_depth = 0;
        let mut current = root;
        loop {
            let page = pager.read_any(current)?;
            match page {
                AnyPage::Leaf(_) => break,
                AnyPage::Branch(branch) => {
                    leaf_depth += 1;
                    current = branch.children[0];
                },
                _ => {
                    return Err(StoreErr::UnexpectedPagetype { 
                        found: page.to_pagetype(), 
                        expected: PageType::Branch
                    });
                },
            }
        }

        let mut prev_key: Option<String> = None;
        loop {
            let page = pager.read::<LeafPage>(current)?;
            for key in page.keys {
                if let Some(prev) = prev_key {
                    if key <= prev {
                        return Err(TreeErr::LeafKeysBadSeq)?;
                    }
                }
                prev_key = Some(key);
            }

            match page.next_leaf {
                Some(x) => current = x,
                None => break,
            }
        }

        return self.validate_page(root, 0, leaf_depth, None, None);
    }

    fn validate_page(&self, id: PageId, depth: usize, leaf_depth: usize,
        min: Option<&str>, max: Option<&str>) -> StoreResult<()> {

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn setup(n: usize) -> (BpTree, Pager) {
        let file = NamedTempFile::new().unwrap();
        let (mut pager, _) = Pager::new(file.path().to_str().unwrap()).unwrap();
        let mut tree = BpTree::new(None);
        for i in 1..n {
            let key = format!("key{}", n);
            tree.insert(&key, Rid { page: PageId::new(i).unwrap(), slot: i as u16 }, &mut pager)
                .unwrap();
        }
        (tree, pager)
    }

    #[test]
    fn insert_until_split() {
        let (tree, mut pager) = setup(300);
        assert!(tree.validate(&mut pager).is_ok())
    }

    #[test]
    fn delete_root() {
    }

    #[test]
    fn delete_leaf_root() {
    }

    #[test]
    fn stress_test() {
    }

    #[test]
    fn delete_until_borrow() {
    }

    #[test]
    fn delete_until_merge() {
    }

    #[test]
    fn delete_until_cascading_merge() {
    }
}
