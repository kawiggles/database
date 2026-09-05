use crate::{
    errors::{DbResult, StoreErr, StoreResult, TreeErr, UserErr},
    store::{
        Rid, RID_SIZE,
        pager::{AnyPage, BranchPage, LeafPage, Page, PageId, PageType, Pager, 
            page::{PAGE_CAPACITY, PAGEID_SIZE, SLOT_POINTER_SIZE}},
    }
};

pub struct BpTree {
    pub root: Option<PageId>,
}

impl BpTree {
    // will eventually need to come up with a method for creating a new bp tree from a list of keys
    // or merging two trees (join operation). That'll be an implementation of merge sort, yay.
    pub fn new(root: Option<PageId>) -> Self {
        BpTree { root }
    }

    // really want to make this a property of the b+ tree for O(1) time
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
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype()))?,
            }
        }
    }

    pub fn contains(&self, key: &str, pager: &mut Pager) -> DbResult<bool> {
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
                     return Ok(leaf.keys
                        .binary_search_by(|probe| { probe.as_str().cmp(key) })
                        .map(|_| { return true; })
                        .unwrap_or(false));
                },
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype()))?,
            }
        }
    }

    fn first_leaf(&self, pager: &mut Pager) -> StoreResult<PageId> {
        let Some(root) = self.root else { return Err(TreeErr::Empty)?; };

        let mut current = root;
        loop {
            let page = pager.read_any(current)?;
            match page {
                AnyPage::Leaf(_) => return Ok(current),
                AnyPage::Branch(branch) => current = branch.children[0],
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype())),
            }
        }
    }

    pub fn scan_rids(&self, pager: &mut Pager) -> DbResult<Vec<Rid>> {
        let mut rids: Vec<Rid> = Vec::new();
        let mut current_leaf = match self.first_leaf(pager) {
            Ok(x) => x,
            Err(StoreErr::TreeErr(TreeErr::Empty)) => return Ok(vec![]),
            Err(x) => return Err(x)?,
        };

        loop {
            let leaf = pager.read::<LeafPage>(current_leaf)?;
            for rid in leaf.rids {
                rids.push(rid.clone());
            }

            match leaf.next_leaf {
                Some(new) => current_leaf = new,
                None => break,
            }
        }

        Ok(rids)
    }

    // Returns Some(Rid) if the associated RID needs to be deleted
    pub fn insert(&mut self, key: &str, rid: Rid, pager: &mut Pager) -> DbResult<Option<Rid>> {
        // Deny inputs that can't fit into a page
        if key.len() + RID_SIZE + SLOT_POINTER_SIZE > PAGE_CAPACITY as usize {
            return Err(UserErr::LongKey(key.into()))?;
        }

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
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype()))?,
            }
        }
        let mut path = path.iter().rev();

        // Second: insert key and rid into leaf
        let mut page = pager.read::<LeafPage>(current)?;
        let replaced = page.insert(key, rid);

        // Third: split the leaf if needed
        if page.free_space() == None {
            let (promoted, new_page) = page.split(pager);
            let new_id = new_page.header().id;

            pager.write(new_page)?;
            pager.write(page)?;

            // Then we promote the key to the parent branch
            if let Some(&parent_id) = path.next() {
                let mut parent = pager.read::<BranchPage>(parent_id)?;
                parent.insert(promoted, new_id);

                if parent.free_space() == None {
                    parent.split(pager, &mut path, self)?;
                }

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
                AnyPage::Leaf(_) => {
                    break;
                },
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype()))?,
            }
        }
        let mut path = path.iter().rev().peekable();

        // Second: delete the key and rid
        let mut leaf = pager.read::<LeafPage>(current)?;
        let removed = leaf.delete(key)?;

        // Third: handle leaf underflow if needed
        if leaf.free_space().unwrap() as usize > (PAGE_CAPACITY as usize / 2) + PAGEID_SIZE {
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
                        parent.refresh_header();

                        pager.write(sibling)?;
                        borrowed = true;
                        break;
                    }
                }

                // If we did borrow, we can write and flush, if not, we gotta do a merge/loop
                if borrowed {
                    pager.write(parent)?;
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
                        parent.refresh_header();
                        pager.write(parent)?;

                        pager.write(leaf)?;
                    } else if pos > 0 { // left sibling
                        let sib_id = parent.children[pos-1];
                        let mut sibling = pager.read::<LeafPage>(sib_id)?;
                        sibling.merge(leaf);
                        pager.free(current)?;

                        parent.keys.remove(pos - 1);
                        parent.children.remove(pos);
                        parent.refresh_header();
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
            if page.free_space().unwrap() as usize > (PAGE_CAPACITY as usize / 2) + PAGEID_SIZE {
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
                            if is_left {
                                let old_sep = parent.keys[pos-1].clone();
                                let borrowed_key = page.borrow_from(&mut sibling, is_left, old_sep);
                                parent.keys[pos-1] = borrowed_key;
                            } else {
                                let old_sep = parent.keys[pos].clone();
                                let borrowed_key = page.borrow_from(&mut sibling, is_left, old_sep);
                                parent.keys[pos] = borrowed_key;
                            }
                            parent.refresh_header();

                            pager.write(sibling)?;
                            borrowed = true;
                            break;
                        }
                    }

                    // If we did borrow, we can write and flush, if not, we gotta do a merge/loop
                    if borrowed {
                        pager.write(parent)?;
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
                            parent.refresh_header();
                            pager.write(parent)?;

                            page.keys.insert(boundary, sep_key);
                            page.refresh_header();

                            pager.write(page)?;
                        } else if pos > 0 { // left sibling
                            let sib_id = parent.children[pos-1];
                            let mut sibling = pager.read::<BranchPage>(sib_id)?;
                            let boundary = sibling.keys.len();
                            sibling.merge(page);
                            pager.free(*id)?;

                            let sep_key = parent.keys.remove(pos - 1);
                            parent.children.remove(pos);
                            parent.refresh_header();
                            pager.write(parent)?;

                            sibling.keys.insert(boundary, sep_key);
                            sibling.refresh_header();

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
                    pager.free(root_id)?;
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
                _ => return Err(StoreErr::UnexpectedPagetype(page.to_pagetype())),
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

                if branch.free_space().unwrap() > ((PAGE_CAPACITY + 1) / 2) + PAGEID_SIZE as u16 
                    && depth != 0 {
                    eprintln!("underflow: id={:?} depth={} keys={}", id, depth, branch.keys.len());
                    return Err(TreeErr::PageUnderflow(id))?;
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

                if leaf.free_space().unwrap() > ((PAGE_CAPACITY + 1) / 2) + PAGEID_SIZE as u16
                    && depth != 0 {
                    eprintln!("underflow: id={:?} depth={} keys={}", id, depth, leaf.keys.len());
                    return Err(TreeErr::PageUnderflow(id))?;
                }
                
                if depth != leaf_depth {
                    return Err(TreeErr::LeafBadDepth(id))?;
                }

                if leaf.rids.len() != leaf.keys.len() {
                    return Err(TreeErr::KeyValueDesync(id))?;
                }

            },
            _ => return Err(StoreErr::UnexpectedPagetype(current.to_pagetype())),
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
    use std::thread;

    fn setup(n: usize) -> (BpTree, Pager) {
        let file = NamedTempFile::new().unwrap();
        let (mut pager, _) = Pager::new(file.path().to_str().unwrap()).unwrap();
        let mut tree = BpTree::new(None);
        for i in 1..n + 1 {
            let key = format!("key{:05}", i);
            tree.insert(&key, Rid { page: PageId::new(i).unwrap(), slot: i as u16 }, &mut pager)
                .unwrap();
        }
        (tree, pager)
    }

    fn scan_keys(tree: &BpTree, pager: &mut Pager) -> StoreResult<(usize, Vec<String>)> {
        let mut keys: Vec<String> = Vec::new();
        let mut len = 0;

        let Some(root) = tree.root else {
            return Ok((len, keys));
        };
        let mut current = root;
        loop {
            match pager.read_any(current)? {
                AnyPage::Leaf(_) => break,
                AnyPage::Branch(branch) => current = branch.children[0],
                other => return Err(StoreErr::UnexpectedPagetype(other.to_pagetype())),
            }
        }

        loop {
            let page = pager.read::<LeafPage>(current)?;
            len += page.keys.len();

            for key in page.keys {
                keys.push(key);
            }

            match page.next_leaf {
                Some(next) => current = next,
                None => break
            }
        }

        Ok((len, keys))
    }

    fn assert_tree_ok(tree: &BpTree, pager: &mut Pager, expected: &[String]) -> StoreResult<()> {
        tree.validate(pager)?;

        let (len, keys) = scan_keys(tree, pager)?;
        assert_eq!(len, expected.len());
        assert_eq!(keys, expected);
        Ok(())
    }

    #[test]
    fn insert_until_split() -> StoreResult<()>{
        let (tree, mut pager) = setup(300);
        tree.print(&mut pager);
        tree.validate(&mut pager)?;
        Ok(())
    }

    #[test]
    fn delete_leaf_root() {
        let (mut tree, mut pager) = setup(1);
        tree.remove("key00001", &mut pager).unwrap();
        assert!(tree.root.is_none());
    }

    #[test]
    fn delete_root() -> StoreResult<()> {
        let (mut tree, mut pager) = setup(3000);
        
        let expected: Vec<String> = (1..=3000).map(|i| format!("key{:05}", i)).collect();

        for i in 1..=3000 {
            tree.remove(&format!("key{:05}", i), &mut pager).unwrap();
            if i % 50 == 0 && tree.root.is_some() {
                assert_tree_ok(&tree, &mut pager, &expected[i..])?;
            }
        }
        assert!(tree.root.is_none());
        Ok(())
    }

    #[test]
    #[ignore]
    fn stress_test() {
        let handles: Vec<_> = (0..31)
            .map(|_| thread::spawn(|| -> DbResult<()> {
                let rand = fastrand::usize(1..10000);
                let (mut tree, mut pager) = setup(rand as usize);
                let mut expected: Vec<String> = (1..=rand).map(|i| format!("key{:05}", i)).collect();

                let operations = fastrand::usize(1000..10000);
                for i in 1..operations {
                    if (rand + operations) % 2 == 0 {
                        let key_num = fastrand::u16(1..10000);
                        let key = format!("key{:05}", key_num);
                        tree.insert(&key, Rid {
                            page: PageId::new(i).unwrap(),
                            slot: i as u16 }, &mut pager)?;
                        match expected.binary_search(&key) {
                            Ok(j) => expected[j] = key,
                            Err(j) => expected.insert(j, key),
                        }
                    } else {
                        let key = format!("key{:05}", fastrand::usize(1..10000));
                        if tree.contains(&key, &mut pager)? {
                            tree.remove(&key, &mut pager)?;
                            expected.remove(expected.binary_search(&key).unwrap());
                        }
                    }

                    assert_tree_ok(&tree, &mut pager, &expected)?;
                }

                Ok(())
            })).collect();

        for handle in handles {
            handle.join().unwrap().unwrap();
        }
    }

    #[test]
    fn delete_intensive() -> StoreResult<()> {
        let (mut tree, mut pager) = setup(50000);
        let mut expected: Vec<String> = (1..=50000).map(|i| format!("key{:05}", i)).collect();

        if let Some(root_id) = tree.root {
            let root = pager.read::<BranchPage>(root_id)?;
            let child = root.children[0];
            let child_header = pager.read_header(child)?;

            assert_eq!(child_header.pagetype, PageType::Branch);
        }

        for i in 1..=50000 {
            tree.remove(&format!("key{:05}", i), &mut pager).unwrap();
            expected.remove(expected.binary_search(&format!("key{:05}", i)).unwrap_or(0));
            if i % 200 == 0 {
                assert_tree_ok(&tree, &mut pager, &expected)?;
            }
        }

        Ok(())
    }
}
