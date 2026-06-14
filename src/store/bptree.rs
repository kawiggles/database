use crate::DbError;
use crate::store::value::Value;
use crate::store::pager::{DataPage, IndexPage, NodeType, Page, PageType, PageHeader, PageId, Pager};

pub struct BpTree {
    root: Option<PageId>,
    order: usize,
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
    pub fn get(&self, key: &str, pager: &mut Pager) -> Result<Value, DbError> {
        let mut current = match self.root {
            Some(x) => x,
            None => return Err(DbError::NoRoot),
        };

        loop {
            let page: IndexPage = Page::read(pager, current)?;
            match &page.node_type {
                NodeType::Branch { children } => {
                    let i = match page.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        // A hit guarentees the right node, because right is always >=
                        Ok(i) => i + 1,
                        // A miss returns the would be index, which is always the target
                        Err(i) => i,
                    };
                    current = children[i];
                },
                NodeType::Leaf { pages, .. } => { 
                    let data: DataPage = match page.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => Page::read(pager, pages[i])?,
                        Err(_) => return Err(DbError::NoValue),
                    };
                    return Ok(data.value);
                }
            }
        }
    }

    pub fn insert(&mut self, key: &str, val: Value, pager: &mut Pager) -> Result<Option<Value>, DbError> {
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
                },
                Err(i) => {
                    page.keys.insert(i, key.to_string());
                    pages.insert(i, data_id);
                }
             }
        }

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
    pub fn remove(&mut self, key: &str, pager: &mut Pager) -> Result<Value, DbError> {
        let return_val = Err(DbError::NoValue);
        // Handle empty tree case
        let Some(root) = self.root else {
            return Err(DbError::NoRoot);
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
                    return_val = Ok(data.value);
                    pager.free(data.page_id());
                }
            },
            Err(_) => return Err(DbError::NoValue),
        }

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
                        }
                    }
                    Page::write(pager, sibling)?;
                }
            } else if right_surplus {
                // For right, everything is popped differently
                let sibling: IndexPage = Page::read(pager, r_sib.unwrap())?;
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

                            let current: IndexPage = Page::read(pager, *idx)?;
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
                pager.free(r_sib.unwrap());
                Page::write(pager, parent)?;

                let current: IndexPage = Page::read(pager, *idx)?;
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
        return_val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bptree_get() {
        let mut tree = BpTree::new(5);
        tree.nodes.push( Node { 
            keys: vec!["key".to_string()], 
            node_type: NodeType::Leaf { 
                values: vec![Value::Int(3)], 
                next: None,
            } 
        });
        assert_eq!(Value::Int(3), tree.get("key").unwrap());
        assert!(tree.validate().is_ok());
    }

    #[test]
    fn bptree_insert() {
        let mut tree = BpTree::new(3);
        tree.insert("one", Value::Int(1));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.insert("two", Value::Int(2));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.insert("three", Value::Int(3));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.insert("four", Value::Int(4));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.insert("five", Value::Int(5));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.insert("six", Value::Int(6));
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }

    #[test]
    fn bptree_stress_test() {
        for order in [3, 4, 5, 10] {
            for n in [10, 20, 50, 100] {
                let mut tree = BpTree::new(order);
                for i in 0..n {
                    tree.insert(&format!("key{:03}", i), Value::Int(i));
                    assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
                }
                // verify all keys retrievable
                for i in 0..n {
                    assert!(tree.get(&format!("key{:03}", i)).is_some());
                }
            }
        }
    }

    fn build_tree(o: usize, n: i64) -> BpTree {
        let mut tree = BpTree::new(o);
        for i in 1..n {
            tree.insert(&format!("key{:03}", i), Value::Int(i));
        }
        tree
    }

    #[test]
    fn bptree_show_tree() {
        let tree = build_tree(4, 16);
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }

    #[test]
    fn bptree_remove_simple() {
        let mut tree = build_tree(3, 20);
        tree.remove("key018");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }

    #[test]
    fn bptree_remove_borrow() {
        let mut tree = build_tree(3, 20);
        tree.remove("key020");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }
    
    #[test]
    fn bptree_remove_merge() {
        let mut tree = build_tree(3, 21);
        tree.remove("key020");
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.remove("key019");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }

    #[test]
    fn bptree_remove_cascade() {
        let mut tree = build_tree(3, 14);
        tree.print_tree();
        tree.remove("key003");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.remove("key006");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.remove("key009");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
        tree.remove("key012");
        tree.print_tree();
        assert!(tree.validate().is_ok(), "Error is: {:?}", tree.validate());
    }
}

#[cfg(test)]
impl BpTree {
    // Assorted functions for displaying information in tests
    fn print_node(&self, node_idx: usize, prefix: &str, is_last: bool) {
        let node = &self.nodes[node_idx];

        print!("{}", prefix);
        if is_last {
            print!("└── ");
        } else {
            print!("├── ");
        }
        
        match &node.node_type {
            NodeType::Leaf { values: _ , next } => {
                let next_str = match next {
                    Some(idx) => format!(" -> [{}]", idx),
                    None => " -> []".to_string(),
                };
                println!("[Leaf: {}] keys: {:?}{}", node_idx, node.keys, next_str);
            },
            NodeType::Branch { children } => {
                println!("[Branch: {}] keys: {:?}", node_idx, node.keys);
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else {"|   "});
                for (i, &child_idx) in children.iter().enumerate() {
                    let child_is_last = i == children.len() - 1;
                    self.print_node(child_idx, &new_prefix, child_is_last);
                }
            },
        }
    }

    fn print_tree(&self) {
        if self.nodes.is_empty() {
            println!("Tree is empty");
            return;
        }

        println!("Tree Structure: (Root: {})", self.root);
        self.print_node(self.root, "", true);
        println!();
        println!();
    }

    fn _print_nodes(&self) {
        if self.nodes.is_empty() {
            println!("Tree is empty");
            return;
        }

        println!("Nodes vector:");
        for (index, node) in self.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Branch { .. } => println!("\t[{}: Branch] keys: {:?}", index, node.keys),
                NodeType::Leaf { .. } => println!("\t[{}: Leaf] keys: {:?}", index, node.keys),
            }
        }
        println!();
    }

    // Function to check if a tree is valid
    fn validate(&self) -> Result<(), TreeErr> {
        if self.nodes.is_empty() {
            return Err(TreeErr::Empty);
        }

        if let NodeType::Branch { children } = &self.nodes[self.root].node_type {
            if children.len() < 2 {
                return Err(TreeErr::RootTooFewChildren);
            }
        }

        // Get leaf depth and traverse to leftmost leaf
        let mut leaf_depth = 0;
        let mut current = self.root;
        loop {
            match &self.nodes[current].node_type {
                NodeType::Branch { children } => {
                    leaf_depth += 1;
                    current = children[0];
                },
                NodeType::Leaf { .. } => break,
            }
        }

        // Check all keys are ordered and that no branches are leaf level
        let mut prev_key: Option<&String> = None;
        loop {
            let node = &self.nodes[current];
            match  &node.node_type {
                NodeType::Leaf { next, .. } => {
                    for key in &node.keys {
                        if let Some(prev) = prev_key {
                            if key <= prev {
                                return Err(TreeErr::LeafKeysBadSeq);
                            }
                        }
                        prev_key = Some(key);
                    }

                    match next {
                        Some(x) => current = *x,
                        None => break,
                    }
                },
                NodeType::Branch { .. } => return Err(TreeErr::BranchInLeafSeq),
            }
        }
        
        // Recurse through each node checking leaves
        return self.validate_node(self.root, 0, leaf_depth, None, None); 
    }

    fn validate_node(&self, idx: usize, depth: usize, leaf_depth: usize, 
        min_bound: Option<&str>, max_bound: Option<&str>) -> Result<(), TreeErr> {
        
        let node = &self.nodes[idx];
        
        // Ensure keys are properly ordered
        let mut iter = node.keys.iter().peekable();
        while let Some(key) = iter.next() {
            if let Some(next_key) = iter.peek() {
                if key >= *next_key {
                    return Err(TreeErr::NodeKeySeqErr);
                }
            }
        }

        // Check all keys are in the bounds defined by the parent node
        for key in &node.keys {
            if let Some(min) = min_bound {
                if key.as_str() <= min { return Err(TreeErr::KeyOOB); }
            }
            if let Some(max) = max_bound {
                if key.as_str() > max { return Err(TreeErr::KeyOOB); }
            }
        }

        // Ensure node is above minimum value
        if idx != self.root && (node.keys.len() < self.order / 2 ||
            node.keys.len() > self.order - 1) {
            return Err(TreeErr::KeyCountErr);
        }

        match &node.node_type {
            NodeType::Branch { children } => {
                // Each branch should have keys + 1 children
                if children.len() != node.keys.len() + 1 {
                    return Err(TreeErr::KeyChildDesync);
                }

                for (i, &child) in children.iter().enumerate() {
                    // Set the new bounds and check children
                    let new_min = if i > 0 { 
                        Some(node.keys[i-1].as_str()) 
                    } else { min_bound };
                    let new_max = if i < node.keys.len() { 
                        Some(node.keys[i].as_str()) 
                    } else { max_bound };
                    return self.validate_node(child, depth + 1, leaf_depth, new_min, new_max);
                }
            },
            NodeType::Leaf { values, .. } => {
                // Make sure leaf is at the proper depth
                if depth != leaf_depth {
                    return Err(TreeErr::LeafBadDepth);
                }

                // Ensure leaves have the same number of keys and values
                if values.len() != node.keys.len() {
                    return Err(TreeErr::KeyValueDesync);
                }

            },
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug)]
enum TreeErr {
    Empty,
    RootTooFewChildren,
    LeafKeysBadSeq,
    BranchInLeafSeq,
    NodeKeySeqErr,
    KeyChildDesync,
    KeyCountErr,
    KeyValueDesync,
    LeafBadDepth,
    KeyOOB,
}

#[cfg(test)]
impl From<TreeErr> for std::fmt::Error {
    fn from(_error: TreeErr) -> Self {
        std::fmt::Error
    }
}
