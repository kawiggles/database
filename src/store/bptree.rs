use crate::{
    errors::{DbResult, StoreErr, StoreResult, TreeErr, UserErr},
    store::{
        Rid,
        pager::{AnyPage, BranchPage, LeafPage, Page, PageId, PageType, Pager, page::PAGE_CAPACITY},
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

    pub fn len(&self, pager: &mut Pager) -> DbResult<usize> {
        let Some(root) = self.root else {
            return Ok(0);
        };

        let mut current = root;
        loop {
            match pager.read_any(current)? {
                AnyPage::Leaf(_) => break,
                AnyPage::Branch(branch) => current = branch.children[0],
                _ => return Err(StoreErr::UnexpectedPagetype {
                    found: pager.read_header(current)?.pagetype,
                    expected: PageType::Branch,
                })?,
            }
        }

        let mut count = 0;
        loop {
            let page = pager.read::<LeafPage>(current)?;
            count += page.keys.len();
            match page.next_leaf {
                Some(next) => current = next,
                None => break,
            }
        }
        Ok(count)
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
                            let boundary = page.keys.len();
                            page.merge(sibling);
                            pager.free(sib_id)?;

                            let sep_key = parent.keys.remove(pos);
                            parent.children.remove(pos + 1);
                            pager.write(parent)?;

                            page.keys.insert(boundary, sep_key);
                            pager.write(page)?;
                        } else if pos > 0 { // left sibling
                            let sib_id = parent.children[pos-1];
                            let mut sibling = pager.read::<BranchPage>(sib_id)?;
                            let boundary = sibling.keys.len();
                            sibling.merge(page);
                            pager.free(current)?;

                            let sep_key = parent.keys.remove(pos - 1);
                            parent.children.remove(pos);
                            pager.write(parent)?;

                            sibling.keys.insert(boundary, sep_key);
                            pager.write(sibling)?;
                        } else {
                            unreachable!("How tf did you get a branch with no siblings and a parent???");
                        }
                    }
                } else { break; }
            } else { break; }
        }

        if let Some(root_id) = self.root {
            let header = pager.read_header(root_id)?;
            if header.pagetype == PageType::Branch {
                let root_page = pager.read::<BranchPage>(root_id)?;
                if root_page.keys.is_empty() {
                    self.root = Some(root_page.children[0]);
                }
            }
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

        return self.validate_page(root, 0, leaf_depth, None, None, pager);
    }

    fn validate_page(&self, id: PageId, depth: usize, leaf_depth: usize,
        min: Option<&str>, max: Option<&str>, pager: &mut Pager) -> StoreResult<()> {
        let current = pager.read_any(id)?;

        match current {
            AnyPage::Branch(branch) => {
                let mut key_iter = branch.keys.iter().peekable();
                while let Some(key) = key_iter.next() {
                    if let Some(minkey) = min {
                        if key.as_str() <= minkey { return Err(TreeErr::KeyOOB(id))?; }
                    }
                    if let Some(maxkey) = max {
                        if key.as_str() > maxkey { return Err(TreeErr::KeyOOB(id))?; }
                    }
                    if let Some(next) = key_iter.peek() {
                        if key >= *next {
                            return Err(TreeErr::NodeKeySeqErr(id))?;
                        }
                    }
                }
                
                if branch.children.len() != branch.keys.len() + 1 {
                    return Err(TreeErr::KeyChildDesync(id))?;
                }

                for (i, &child) in branch.children.iter().enumerate() {
                    let new_min = if i > 0 {
                        Some(branch.keys[i-1].as_str())
                    } else { min };
                    let new_max = if i < branch.keys.len() {
                        Some(branch.keys[i].as_str())
                    } else { max };
                    self.validate_page(child, depth+1, leaf_depth, new_min, new_max, pager)?;
                }
            },
            AnyPage::Leaf(leaf) => {
                let mut key_iter = leaf.keys.iter().peekable();
                while let Some(key) = key_iter.next() {
                    if let Some(min) = min {
                        if key.as_str() < min { return Err(TreeErr::KeyOOB(id))?; }
                    }
                    if let Some(max) = max {
                        if key.as_str() >= max { return Err(TreeErr::KeyOOB(id))?; }
                    }
                    if let Some(next) = key_iter.peek() {
                        if key >= *next {
                            return Err(TreeErr::NodeKeySeqErr(id))?;
                        }
                    }
                }
                
                if depth != leaf_depth {
                    return Err(TreeErr::LeafBadDepth(id))?;
                }

                if leaf.rids.len() != leaf.keys.len() {
                    return Err(TreeErr::KeyValueDesync(id))?;
                }

            },
            _ => { return Err(StoreErr::UnexpectedPagetype {
                found: current.to_pagetype(),
                expected: PageType::Branch })?;
            },
        }

        Ok(())
    }

    fn print_page(&self, page_id: PageId, prefix: &str, is_last: bool, pager: &mut Pager) {
        print!("{}", prefix);
        if is_last {
            print!("└── ");
        } else {
            print!("├── ");
        }
        
        let header = pager.read_header(page_id).unwrap();
        match header.pagetype {
            PageType::Leaf => {
                let next_str = match header.next {
                    Some(idx) => format!(" -> (id: {:?})", idx),
                    None => " -> []".to_string(),
                };
                let page = pager.read::<LeafPage>(page_id).unwrap();
                println!("Leaf(id: {:?}, keys: {:?}){}", page_id, page.keys, next_str);
            },
            PageType::Branch => {
                let page = pager.read::<BranchPage>(page_id).unwrap();
                println!("Branch(id: {:?}, keys: {:?})", page_id, page.keys);
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else {"|   "});
                for (i, &child_idx) in page.children.iter().enumerate() {
                    let child_is_last = i == page.children.len() - 1;
                    self.print_page(child_idx, &new_prefix, child_is_last, pager);
                }
            },
            _ => return,
        }
    }

    pub fn print(&self, pager: &mut Pager) {
        println!();
        let Some(root) = self.root else {
            println!("Tree is empty");
            println!();
            return;
        };

        println!("Root (id: {:?})", root);
        self.print_page(root, "", true, pager);
        println!();
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
        for i in 1..n + 1 {
            let key = format!("longerkey{}", i);
            tree.insert(&key, Rid { page: PageId::new(i).unwrap(), slot: i as u16 }, &mut pager)
                .unwrap();
        }
        (tree, pager)
    }

    #[test]
    fn insert_until_split() {
        let (tree, mut pager) = setup(300);
        tree.print(&mut pager);
        assert!(tree.validate(&mut pager).is_ok())
    }

    #[test]
    fn delete_root() {
        let (mut tree, mut pager) = setup(161);
        print!("{}", tree.len(&mut pager).unwrap());
        for i in 0..tree.len(&mut pager).unwrap() {
            tree.remove(&format!("longerkey{}", i + 1), &mut pager).unwrap();
        }
        tree.print(&mut pager);
    }

    #[test]
    fn delete_leaf_root() {
        let (mut tree, mut pager) = setup(2);
        tree.remove("longerkey1", &mut pager).unwrap();
        assert!(tree.root.is_none());
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
