use crate::{
    store::{
        RID,
        value::Value,
        pager::{ Pager, Page, PageId,  PageType, DataPage },
        BranchPage, LeafPage, 
    },
    errors::{DbErr, DbResult, UserErr, StoreErr },
};

pub struct BpTree {
    pub root: Option<PageId>,
    pub order: usize,
}

impl BpTree {
    // Easiest method on the tree
    pub fn new(root: Option<PageId>, order: usize) -> Self {
        BpTree {
            root,
            order 
        }
    }

    // This function shows the basic pattern for searching the tree with a key
    pub fn get(&self, key: &str, pager: &mut Pager) -> DbResult<RID> {
        let mut current = match self.root {
            Some(x) => x,
            None => return Err(DbErr::UserErr(UserErr::NoRoot)),
        };

        loop {
            let header = pager.read_header(current)?;
            match header.pagetype {
                PageType::Branch => {
                    let branch = pager.read::<BranchPage>(current)?;
                    let i = match branch.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        // A hit guarentees the right node, because right is always >=
                        Ok(i) => i + 1,
                        // A miss returns the would be index, which is always the target
                        Err(i) => i,
                    };
                    current = branch.children[i];
                },
                PageType::Leaf => { 
                    let leaf = pager.read::<LeafPage>(current)?;
                    let index = leaf.keys.binary_search_by(|probe| {
                        probe.as_str().cmp(key)
                    }).map_err(|_| UserErr::NoRID(key.into()))?;

                    return Ok(leaf.rids[index]);
                },
                _ => { 
                    return Err(DbErr::StoreErr(StoreErr::UnexpectedPagetype{
                        found: header.pagetype,
                        expected: PageType::Branch,
                    }));
                }
            }
        }
    }

    pub fn insert(&mut self, key: &str, val: Value, pager: &mut Pager) -> DbResult<Option<Value>> {
        let mut return_val = None;

        // Create a DataPage and write the value to it
        let data_id = DataPage::new(pager, val)?;
        
        // If the tree is empty, create a new root
        let Some(root) = self.root else {
            let new_id = pager.alloc();
            let page = IndexPage::new_leaf(new_id, vec![key.to_string()], vec![data_id], None);
            Page::write(pager, page)?;
            pager.flush()?;

            self.root = Some(new_id);
            return Ok(None);
        };

        let mut path: Vec<PageId> = Vec::new(); // for tracking nodes to edit if split is needed

        // First: find the leaf node while tracking path
        let mut current = root;
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
                },
                NodeType::Leaf { .. } => {
                    path.push(current);
                    break;
                }
            }
        }

        // Second: insert key into node
        let mut page: IndexPage = Page::read(pager, current)?;
        if let NodeType::Leaf { pages, .. } = &mut page.node_type {
             match page.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                Ok(i) => {
                    let data: DataPage = Page::read(pager, pages[i])?;
                    return_val = Some(data.value);
                    page.keys[i] = key.to_string();
                    pages[i] = data_id;
                    pager.free(pages[i])?;
                },
                Err(i) => {
                    page.keys.insert(i, key.to_string());
                    pages.insert(i, data_id);
                }
             }
        }
        Page::write(pager, page)?;

        // Third: handle splits, iterating through path
        let mut path_iter = path.iter().rev().peekable();
        while let Some(index) = path_iter.next() {
            // First check if a split is necessary
            let split_result = {
                let mut page: IndexPage = Page::read(pager, *index)?;
                if page.keys.len() >= self.order { // This is where max keys is defined
                    let mut new_page = match &mut page.node_type {
                        NodeType::Leaf { pages, next } => {
                            let mid = (page.keys.len() + 1) / 2; // ⌈m/2⌉ 
                            let new_keys = page.keys.split_off(mid);
                            let new_values = pages.split_off(mid);
                            // TODO: Fix bug, the fix is pattern matching 
                            let old_next = *next;
                            let new_id = pager.alloc();
                            *next = Some(new_id);
                            IndexPage::new_leaf(new_id, new_keys, new_values, old_next)
                        },
                        NodeType::Branch { children } => {
                            let mid = page.keys.len() / 2; // m/2 for branches 
                            let new_keys = page.keys.split_off(mid); 
                            // increment by 1 because there are 1 more children than keys
                            let new_children = children.split_off(mid + 1);
                            IndexPage::new_branch(pager.alloc(), new_keys, new_children)
                        }
                    };

                    // The promoted key is the key that'll get pushed up to the parent
                    let promoted = match &mut new_page.node_type {
                        NodeType::Leaf { .. } => new_page.keys[0].clone(),
                        NodeType::Branch { .. } => new_page.keys.remove(0),
                    };

                    Page::write(pager, page)?;
                    Some((promoted, new_page))
                } else {
                    None
                }
            };

            // If it is, insert the promoted key and new node into the parent and node vector
            if let Some((promoted, new_page)) = split_result {
                let new_page_id = new_page.page_id();
                Page::write(pager, new_page)?;

                // The parent is the next node in the path (since the iterator was reversed)
                if let Some(&parent_id) = path_iter.peek() {
                    let mut parent: IndexPage = Page::read(pager, *parent_id)?;
                    let i = parent.keys.binary_search_by(|probe| probe.as_str().cmp(&promoted))
                        .unwrap_or_else(|i| i);
                    parent.keys.insert(i, promoted);

                    if let NodeType::Branch { children } = &mut parent.node_type {
                        children.insert(i + 1, new_page_id);
                    }
                    Page::write(pager, parent)?;
                } else {
                    // If there's no parent, we make a new root
                    let parent = IndexPage::new_branch(
                        pager.alloc(), vec![promoted], vec![*index, new_page_id]);
                    self.root = Some(parent.page_id());
                    Page::write(pager, parent)?;
                }
            }
        }

        pager.flush()?;
        Ok(return_val)
    }

    // Holy fucking shit (Tool reference)
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
    use crate::store::Store;

    fn setup() -> Store {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        Store::start(path, 4).unwrap()
    }

    #[test]
    fn bptree_insert_and_get() {
        let mut store = setup();
        store.datamap.insert("one", Value::Int(1), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.datamap.insert("two", Value::Int(2), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.datamap.insert("three", Value::Int(3), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.datamap.insert("four", Value::Int(4), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.datamap.insert("five", Value::Int(5), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.datamap.insert("six", Value::Int(6), &mut store.pager).unwrap();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_stress_test() {
        for n in [10, 20, 50, 100] {
            let mut store = setup();
            for i in 0..n {
                store.datamap.insert(&format!("key{:03}", i), Value::Int(i), &mut store.pager).unwrap();
                assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
            }
            // verify all keys retrievable
            for i in 0..n {
                assert!(store.datamap.get(&format!("key{:03}", i), &mut store.pager).is_ok());
            }
        }
    }

    fn build_store(n: isize) -> Store {
        let mut store = setup();
        println!("{}", store.datamap.order);
        for i in 1..n {
            store.datamap.insert(&format!("key{:03}", i), Value::Int(i), &mut store.pager).unwrap();
        }
        store
    }

    #[test]
    fn bptree_show_tree() {
        let store = build_store(16);
        store.print_tree();
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_remove_simple() {
        let mut store = build_store(20);
        store.print_tree();
        let _ = store.datamap.remove("key018", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_remove_borrow() {
        let mut store = build_store(20);
        store.print_tree();
        let _ = store.datamap.remove("key015", &mut store.pager);
        let _ = store.datamap.remove("key014", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }
    
    #[test]
    fn bptree_remove_merge() {
        let mut store = build_store(21);
        store.print_tree();
        let _ = store.datamap.remove("key020", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        let _ = store.datamap.remove("key019", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }

    #[test]
    fn bptree_remove_cascade() {
        let mut store = build_store(14);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        store.print_tree();
        let _ = store.datamap.remove("key003", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        let _ = store.datamap.remove("key006", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        let _ = store.datamap.remove("key009", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
        let _ = store.datamap.remove("key012", &mut store.pager);
        assert!(store.validate().is_none(), "Error is: {:?}", store.validate());
    }
}
