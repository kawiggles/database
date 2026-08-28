use crate::{
    errors::{DbErr, DbResult, StoreErr, UserErr },
    store::{
        Rid,
        pager::{ AnyPage, DataPage, LeafPage, BranchPage, Page, PageId, PageType, Pager },
        value::Value 
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
            None => return Err(DbErr::UserErr(UserErr::NoRoot)),
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
                    return Err(DbErr::StoreErr(StoreErr::UnexpectedPagetype{
                        found: page.to_pagetype(),
                        expected: PageType::Branch,
                    }));
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
                    return Err(DbErr::StoreErr(StoreErr::UnexpectedPagetype{
                        found: page.to_pagetype(),
                        expected: PageType::Branch,
                    }));
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
    pub fn remove(&mut self, key: &str, pager: &mut Pager) -> DbResult<Value> {
        let mut return_val: Option<Value> = None;
        // Handle empty tree case
        let Some(root) = self.root else {
            return Err(UserErr::NoRoot)?;
        };

        // First, search for the leaf node with the key to delete
        let mut current = root;
        let mut path = Vec::new();
        loop {
            let page: IndexPage = Page::read(pager, current)?;
            match &page.node_type {
                NodeType::Branch { children } => {
                    path.push(current);
                    let i = match page.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => i + 1,
                        Err(i) => i,
                    };
                    current = children[i];
                }
                NodeType::Leaf { .. } => {
                    path.push(current);
                    break;
                },
            }
        }

        // Second, delete the key and shift the key vector
        let mut page: IndexPage = Page::read(pager, current)?;
        match page.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
            Ok(i) => {
                page.keys.remove(i);
                if let NodeType::Leaf { pages , .. } = &mut page.node_type {
                    let data: DataPage = Page::read(pager, pages.remove(i))?;
                    pager.free(data.page_id())?;
                    return_val = Some(data.value);
                }
            },
            Err(_) => return Err(UserErr::NoValue)?,
        }
        Page::write(pager, page)?;

        // Third, handle underflow vectors
        let mut path_iter = path.iter().rev().peekable();
        // Like insertion, iterating through every visited node
        while let Some(idx) = path_iter.next() {
            // The parent node is important for retrieving and storing separator keys
            let parent_idx = match path_iter.peek() {
                Some(&&p) => p,
                // If prior operations destroy the root, then create a new one from the children
                None => {
                    let root_page: IndexPage = Page::read(pager, root)?;
                    if root_page.keys.is_empty() {
                        if let NodeType::Branch { children } = &root_page.node_type {
                            if children.len() == 1 {
                                self.root = Some(children[0]);
                            }
                        }
                    }
                    break;
                },
            };

            // Check if underflow occured and find siblings/pos
            let (min_keys, rebalance, pos, l_sib, r_sib, is_leaf) = {
                let page: IndexPage = Page::read(pager, *idx)?;
                // Need to know if leaf or branch, because operations differ depending on type
                let is_leaf = matches!(page.node_type, NodeType::Leaf { .. });
                let min_keys = self.order / 2;

                if page.keys.len() >= min_keys {
                    (min_keys, false, 0, None, None, is_leaf)
                } else {
                    // from the parent, we grab...
                    let parent: IndexPage = Page::read(pager, parent_idx)?;
                    if let NodeType::Branch { children } = &parent.node_type {
                        // ...the nodes position in the children vector
                        let pos = children.iter().position(|&c| c == *idx).unwrap();
                        // ...and it's siblings
                        let left_sib = if pos > 0 { Some(children[pos - 1]) } else { None };
                        let right_sib = if pos < children.len() - 1 { 
                            Some(children[pos+1]) } else { None };
                        (min_keys, true, pos, left_sib, right_sib, is_leaf)
                    } else { panic!("You somehow have a parent that's a leaf node") }
                }
            };

            // Need to break loop out here because borrow checker
            if !rebalance { break; }

            // Ignore this sketchy code
            let left_surplus = l_sib.map_or(false, |s| {
                IndexPage::read(pager, s).unwrap().keys.len() > min_keys});
            let right_surplus = r_sib.map_or(false, |s| {
                IndexPage::read(pager, s).unwrap().keys.len() > min_keys});

            // Attempt the following in order: left borrow, right borrow, right merge, left merge
            if left_surplus {
                let mut sibling: IndexPage = Page::read(pager, l_sib.unwrap())?; // assume l_sib
                // The sibling has to have enough keys to borrow hence > and not >=
                if sibling.keys.len() > min_keys {
                    if is_leaf {
                        // We pop the last key and value, because left
                        let borrow_key = sibling.keys.remove(sibling.keys.len() - 1);
                        let borrow_val = {
                            if let NodeType::Leaf { pages, .. } = &mut sibling.node_type {
                                Some(pages.remove(pages.len() - 1))
                            } else { None }
                        };

                        // We insert both into the first position of our node
                        let mut current: IndexPage = Page::read(pager, *idx)?;
                        current.keys.insert(0, borrow_key.clone());
                        if let Some(val) = borrow_val {
                            if let NodeType::Leaf { pages, .. } = &mut current.node_type {
                                pages.insert(0, val);
                            }
                        }
                        Page::write(pager, current)?;

                        // And then update the parent separator
                        let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                        parent.keys[pos-1] = borrow_key.clone();
                        Page::write(pager, parent)?;
                    } else {
                        // Branches have separate logic
                        // First we pop the key and child from the sibling
                        let borrow = {
                            if let NodeType::Branch { children } = &mut sibling.node_type {
                                let key = sibling.keys.remove(sibling.keys.len() - 1);
                                let child = children.remove(children.len() - 1);
                                Some((key, child))
                            } else { None }
                        };

                        if let Some((new_key, new_child)) = borrow {
                            // Take separator...
                            let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                            let sep_key = parent.keys[pos-1].clone();
                            // insert sibling's key into parent in separator position
                            parent.keys[pos-1] = new_key;
                            Page::write(pager, parent)?;

                            let mut current: IndexPage = Page::read(pager, *idx)?;
                            // ...and insert it into the current node
                            current.keys.insert(0, sep_key);
                            // and insert the child from the sibling
                            if let NodeType::Branch { children } = &mut current.node_type {
                                children.insert(0, new_child);
                            }
                            Page::write(pager, current)?;
                        }
                    }
                    Page::write(pager, sibling)?;
                }
            } else if right_surplus {
                // For right, everything is popped differently
                let mut sibling: IndexPage = Page::read(pager, r_sib.unwrap())?;
                if sibling.keys.len() > min_keys {
                    if is_leaf {
                        let borrow_key = sibling.keys.remove(0);
                        let borrow_val = {
                            if let NodeType::Leaf { pages, .. } = &mut sibling.node_type {
                                Some(pages.remove(0))
                            } else { None }
                        };

                        let mut current: IndexPage = Page::read(pager, *idx)?;
                        current.keys.push(borrow_key.clone());
                        if let Some(val) = borrow_val {
                            if let NodeType::Leaf { pages, .. } = &mut current.node_type {
                                pages.push(val);
                            }
                        }
                        Page::write(pager, current)?;

                        // Also, the new separator isn't the borrowed key
                        let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                        let new_sep = sibling.keys[0].clone();
                        parent.keys[pos] = new_sep;
                        Page::write(pager, parent)?;
                    } else {
                        // Logic for right branches is nearly identical, save pos and where pops go
                        let borrow = {
                            if let NodeType::Branch { children } = &mut sibling.node_type {
                                let key = sibling.keys.remove(0);
                                let child = children.remove(0);
                                Some((key, child))
                            } else { None }
                        };

                        if let Some((new_key, new_child)) = borrow {
                            let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                            let sep_key = parent.keys[pos].clone();
                            parent.keys[pos] = new_key;
                            Page::write(pager, parent)?;

                            let mut current: IndexPage = Page::read(pager, *idx)?;
                            current.keys.push(sep_key);
                            if let NodeType::Branch { children } = &mut current.node_type {
                                children.push(new_child);
                            }
                            Page::write(pager, current)?;
                        }
                    }
                    Page::write(pager, sibling)?;
                }
            } else if r_sib.is_some() {
                // If borrowing isn't possible, we attempt merging with the right node first
                // With right merge, we destroy the right node and push it's values to the back of
                // the current node
                let (old_keys, old_children, old_values, old_next) = {
                    let mut sibling: IndexPage = Page::read(pager, r_sib.unwrap())?;
                    let keys = sibling.keys.drain(..).collect::<Vec<_>>();

                    match &mut sibling.node_type {
                        NodeType::Branch { children } => {
                            let old_children = children.drain(..).collect::<Vec<_>>();
                            (keys, Some(old_children), None, None)
                        },
                        NodeType::Leaf { pages, next } => {
                            let old_values = pages.drain(..).collect::<Vec<_>>();
                            (keys, None, Some(old_values), Some(*next))
                        },
                    }
                };

                // Then we grab the separator key
                let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                let sep_key = parent.keys.remove(pos);
                // And remove the record of the sibling
                if let NodeType::Branch { children } = &mut parent.node_type {
                    children.remove(pos + 1);
                }
                pager.free(r_sib.unwrap())?;
                Page::write(pager, parent)?;

                let mut current: IndexPage = Page::read(pager, *idx)?;
                match &mut current.node_type {
                    NodeType::Leaf { pages, next } => {
                        if let Some(old_values) = old_values {
                            pages.extend(old_values);
                            *next = old_next.unwrap();
                        }
                    },
                    NodeType::Branch { children } => {
                        if let Some(old_children) = old_children {
                            current.keys.push(sep_key);
                            children.extend(old_children);
                        }
                    },
                }
                // This comes after the match in case the node is a branch, so the sep_key goes in
                // between the two branches' keys
                current.keys.extend(old_keys);
                Page::write(pager, current)?;
            } else if l_sib.is_some() {
                // Same logic for left but we destroy the current node instead
                let (old_keys, old_children, old_values, old_next) = {
                    let mut current: IndexPage = Page::read(pager, *idx)?;
                    let keys = current.keys.drain(..).collect::<Vec<_>>();

                    match &mut current.node_type {
                        NodeType::Branch { children } => {
                            let old_children = children.drain(..).collect::<Vec<_>>();
                            (keys, Some(old_children), None, None)
                        },
                        NodeType::Leaf { pages, next } => {
                            let old_values = pages.drain(..).collect::<Vec<_>>();
                            (keys, None, Some(old_values), Some(*next))
                        },
                    }
                };

                let mut parent: IndexPage = Page::read(pager, parent_idx)?;
                let sep_key = parent.keys.remove(pos-1);
                if let NodeType::Branch { children } = &mut parent.node_type {
                    children.remove(pos);
                }
                pager.free(*idx)?;
                Page::write(pager, parent)?;

                let mut sibling: IndexPage = Page::read(pager, l_sib.unwrap())?;
                match &mut sibling.node_type {
                    NodeType::Leaf { pages, next } => {
                        if let Some(old_values) = old_values {
                            pages.extend(old_values);
                        }
                        *next = old_next.unwrap();
                    },
                    NodeType::Branch { children } => {
                        if let Some(old_children) = old_children {
                            sibling.keys.push(sep_key);
                            children.extend(old_children);
                        }
                    },
                }
                sibling.keys.extend(old_keys);
                Page::write(pager, sibling)?;
            }
        }
        pager.flush()?;
        if let Some(val) = return_val {
            Ok(val)
        } else {
            Err(UserErr::BadDel)?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::{errors::StoreResult, store::Store};

    fn setup() -> Store {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        Store::start(path).unwrap()
    }

    #[test]
    fn bptree_insert_and_get() {
        let mut store = setup();
        store.tree.insert("one", Value::Int(1), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.tree.insert("two", Value::Int(2), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.tree.insert("three", Value::Int(3), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.tree.insert("four", Value::Int(4), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.tree.insert("five", Value::Int(5), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.tree.insert("six", Value::Int(6), &mut store.pager).unwrap();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_stress_test() {
        for n in [10, 20, 50, 100] {
            let mut store = setup();
            for i in 0..n {
                store.tree.insert(&format!("key{:03}", i), Value::Int(i), &mut store.pager).unwrap();
                assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
            }
            // verify all keys retrievable
            for i in 0..n {
                assert!(store.tree.get(&format!("key{:03}", i), &mut store.pager).is_ok());
            }
        }
    }

    fn build_store(n: isize) -> Store {
        let mut store = setup();
        for i in 1..n {
            store.tree.insert(&format!("key{:03}", i), Value::Int(i), &mut store.pager).unwrap();
        }
        store
    }

    #[test]
    fn bptree_show_tree() {
        let mut store = build_store(16);
        store.print_tree();
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_remove_simple() {
        let mut store = build_store(20);
        store.print_tree();
        let _ = store.tree.remove("key018", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
    }

    // TODO: use this pattern for tests
    #[test]
    fn bptree_remove_borrow() -> StoreResult<()> {
        let mut store = build_store(20);
        store.print_tree();
        let _ = store.tree.remove("key015", &mut store.pager);
        let _ = store.tree.remove("key014", &mut store.pager);
        store.validate()?;
        Ok(())
    }
    
    #[test]
    fn bptree_remove_merge() {
        let mut store = build_store(21);
        store.print_tree();
        let _ = store.tree.remove("key020", &mut store.pager);
        store.validate();
        let _ = store.tree.remove("key019", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_remove_cascade() {
        let mut store = build_store(14);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        store.print_tree();
        let _ = store.tree.remove("key003", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        let _ = store.tree.remove("key006", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        let _ = store.tree.remove("key009", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
        let _ = store.tree.remove("key012", &mut store.pager);
        assert!(store.validate().is_ok(), "Error is: {:?}", store.validate());
    }
}
