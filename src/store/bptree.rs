use crate::store::value::Value;

pub enum NodeType {
    Branch { children: Vec<usize> },
    Leaf { values: Vec<Value>, next: Option<usize> },
}

pub struct Node {
    keys: Vec<String>,
    node_type: NodeType,
}

pub struct BpTree {
    nodes: Vec<Node>, // nodes are reference by index to prevent weird borrow checker problems
    root: usize,
    order: usize,
}

impl BpTree {
    // Easiest method on the tree
    pub fn new(order: usize) -> Self {
        BpTree { 
            nodes: Vec::new(), 
            root: 0, 
            order 
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let mut current = self.root;
        loop {
            let node = &self.nodes[current];
            match &node.node_type {
                NodeType::Branch { children } => {
                    let i = match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => i + 1,
                        Err(i) => i,
                    };
                    current = children[i];
                },
                NodeType::Leaf { values, .. } => {
                    return match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => Some(values[i].clone()),
                        Err(_) => None
                    }
                }
            }
        }
    }

    pub fn insert(&mut self, key: &str, val: Value) -> Option<Value> {
        let mut return_val = None;
        
        if self.nodes.is_empty() {
            let node = Node {
                keys: vec![key.to_string()],
                node_type: NodeType::Leaf { 
                    values: vec![val], 
                    next: None, 
                },
            };
            self.nodes.push(node);
            return None;
        }

        let mut path: Vec<usize> = Vec::new(); // for tracking nodes to edit if split is needed

        // First: find the leaf node while tracking path
        let mut current = self.root;
        loop {
            let node = &self.nodes[current];
            match &node.node_type {
                NodeType::Branch { children } => {
                    path.push(current);
                    let i = match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        // This might be wrong, so if there are weird fetches the problem is here
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
        let node = &mut self.nodes[current];
        if let NodeType::Leaf { values, .. } = &mut node.node_type {
             match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                Ok(i) => {
                    node.keys[i] = key.to_string();
                    return_val = Some(val.clone());
                    values[i] = val;
                },
                Err(i) => {
                    node.keys.insert(i, key.to_string());
                    values.insert(i, val);
                }
             }
        }

        // Third: handle splits, iterating through path
        let mut path_iter = path.iter().rev().peekable();
        while let Some(index) = path_iter.next() {
            // First check if a split is necessary
            let split_result = {
                let nodes_len = self.nodes.len();
                let node = &mut self.nodes[*index];

                if node.keys.len() >= self.order { // This is where max keys is defined
                    let mid = (node.keys.len() + 1) / 2; // ⌈m/2⌉ 
                    let new_keys = node.keys.split_off(mid); // Because 0 index shifted one right

                    let mut new_node = match &mut node.node_type {
                        NodeType::Leaf { values, next } => {
                            let new_values = values.split_off(mid);
                            let old_next = *next;
                            *next = Some(nodes_len);
                            Node {
                                keys: new_keys,
                                node_type: NodeType::Leaf { 
                                    values: new_values, 
                                    next: old_next, 
                                }
                            }
                        },
                        NodeType::Branch { children } => {
                            let new_children = children.split_off(mid + 1);
                            Node {
                                keys: new_keys,
                                node_type: NodeType::Branch { 
                                    children: new_children, 
                                }
                            }
                        }
                    };

                    let promoted = match &mut new_node.node_type {
                        NodeType::Leaf { .. } => new_node.keys[0].clone(),
                        NodeType::Branch { .. } => new_node.keys.remove(0),
                    };

                    Some((promoted, new_node))
                } else {
                    None
                }
            };

            // If it is, insert the promoted key and new node into the parent and node vector
            if let Some((promoted, new_node)) = split_result {
                let new_node_idx = self.nodes.len();
                self.nodes.push(new_node);

                if let Some(&parent_idx) = path_iter.peek() {
                    let parent = &mut self.nodes[*parent_idx];
                    let i = parent.keys.binary_search_by(|probe| probe.as_str().cmp(&promoted))
                        .unwrap_or_else(|i| i);
                    parent.keys.insert(i, promoted);

                    if let NodeType::Branch { children } = &mut parent.node_type {
                        children.insert(i + 1, new_node_idx);
                    }
                } else {
                    let parent = Node {
                        keys: vec![promoted],
                        node_type: NodeType::Branch { 
                            children: vec![*index, new_node_idx]
                        }
                    };

                    self.nodes.push(parent);
                    self.root = self.nodes.len() - 1;
                }
            }
        }

        return_val
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let mut return_val = None;

        // Handle empty tree case
        if self.nodes.is_empty() {
            return None;
        }

        // First, search for the leaf node with the key to delete
        let mut current = self.root;
        let mut path = Vec::new();
        loop {
            let node = &self.nodes[current];
            match &node.node_type {
                NodeType::Branch { children } => {
                    path.push(current);
                    let i = match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
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
        let node = &mut self.nodes[current];
        match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
            Ok(i) => {
                node.keys.remove(i);
                if let NodeType::Leaf { values, .. } = &mut node.node_type {
                    return_val = Some(values.remove(i));
                }
            },
            Err(_) => return None,
        }

        // Third, handle underflow vectors
        let mut path_iter = path.iter().rev().peekable();
        while let Some(idx) = path_iter.next() {
            let parent_idx = match path_iter.peek() {
                Some(&&p) => p,
                None => {
                    if self.nodes[self.root].keys.is_empty() {
                        if let NodeType::Branch { children } = &self.nodes[self.root].node_type {
                            if children.len() == 1 {
                                self.root = children[0];
                            }
                        }
                    }
                    break;
                },
            };

            // Check if underflow occured and find siblings/pos
            let (min_keys, rebalance, pos, l_sib, r_sib, is_leaf) = {
                let node = &self.nodes[*idx];
                let is_leaf = matches!(node.node_type, NodeType::Leaf { .. });
                let min_keys = match node.node_type {
                    NodeType::Branch { .. } => (self.order + 1) / 2,
                    NodeType::Leaf { .. } => self.order / 2,
                };

                if node.keys.len() >= min_keys {
                    (min_keys, false, 0, None, None, is_leaf)
                } else {
                    let parent = &self.nodes[parent_idx];
                    if let NodeType::Branch { children } = &parent.node_type {
                        let pos = children.iter().position(|&c| c == *idx).unwrap();
                        let left_sib = if pos > 0 { Some(children[pos - 1]) } else { None };
                        let right_sib = if pos < children.len() - 1 { 
                            Some(children[pos+1]) } else { None };
                        (min_keys, true, pos, left_sib, right_sib, is_leaf)
                    } else { unreachable!() }
                }
            };

            if !rebalance { break; }

            // Attempt borrow, then merge
            if let Some(sib_idx) = l_sib {
                let sibling = &mut self.nodes[sib_idx];
                if sibling.keys.len() > min_keys {
                    if is_leaf {
                        let borrow_key = sibling.keys.remove(sibling.keys.len() - 1);
                        let borrow_val = {
                            if let NodeType::Leaf { values, .. } = &mut sibling.node_type {
                                Some(values.remove(values.len() - 1))
                            } else { None }
                        };

                        self.nodes[*idx].keys.insert(0, borrow_key.clone());

                        if let Some(val) = borrow_val {
                            if let NodeType::Leaf { values, .. } = &mut self.nodes[*idx].node_type {
                                values.insert(0, val);
                            }
                        }

                        let parent = &mut self.nodes[parent_idx];
                        parent.keys[pos-1] = borrow_key.clone();
                    } else {
                        // Branch has separate logic
                        let sibling = &mut self.nodes[sib_idx];
                        let borrow = { // pop key and child from sibling
                            if let NodeType::Branch { children } = &mut sibling.node_type {
                                let key = sibling.keys.remove(sibling.keys.len() - 1);
                                let val = children.remove(children.len() - 1);
                                Some((key, val))
                            } else { None }
                        };

                        // Take separator and insert into current node
                        let parent = &self.nodes[parent_idx];
                        let sep_key = 

                        // Put the left siblings key into the separator spot of the parent

                        // Insert the popped child into the current node's children
                    }
                }
            } else if let Some(sib_idx) = r_sib {
                let sibling = &mut self.nodes[sib_idx];
                if sibling.keys.len() > min_keys {
                    if is_leaf {
                        let borrow_key = sibling.keys.remove(0);
                        let sep_key = sibling.keys[0].clone();
                        let borrow_val = {
                            if let NodeType::Leaf { values, .. } = &mut sibling.node_type {
                                Some(values.remove(values.len() - 1))
                            } else { None }
                        };

                        let node_ref = &mut self.nodes[*idx];
                        node_ref.keys.insert(node_ref.keys.len(), borrow_key.clone());

                        if let Some(val) = borrow_val {
                            if let NodeType::Leaf { values, .. } = &mut self.nodes[*idx].node_type {
                                values.insert(0, val);
                            }
                        }

                        let parent = &mut self.nodes[parent_idx];
                        parent.keys[pos-1] = sep_key;
                    } else {
                    }
                }
            } else if let Some(sib_idx) = r_sib {
                let (old_keys, old_children, old_values, old_next) = {
                    let node = &mut self.nodes[sib_idx];
                    let keys = node.keys.drain(..).collect::<Vec<_>>();

                    match &mut node.node_type {
                        NodeType::Branch { children } => {
                            let old_children = children.drain(..).collect::<Vec<_>>();
                            (keys, Some(old_children), None, None)
                        },
                        NodeType::Leaf { values, next } => {
                            let old_values = values.drain(..).collect::<Vec<_>>();
                            (keys, None, Some(old_values), Some(*next))
                        },
                    }
                };

                let parent = &self.nodes[parent_idx];
                let sep_key = parent.keys[pos].clone();
                let merge_node = &mut self.nodes[*idx];

                match &mut merge_node.node_type {
                    NodeType::Leaf { values, next } => {
                        if let Some(old_values) = old_values {
                            values.extend(old_values);
                            *next = old_next.unwrap();
                        }
                    },
                    NodeType::Branch { children } => {
                        if let Some(old_children) = old_children {
                            merge_node.keys.push(sep_key);
                            children.extend(old_children);
                        }
                    },
                }
                merge_node.keys.extend(old_keys);

                let parent = &mut self.nodes[parent_idx];
                parent.keys.remove(pos);
                if let NodeType::Branch { children } = &mut parent.node_type {
                    children.remove(pos + 1);
                }
            } else if let Some(sib_idx) = l_sib {
                let (old_keys, old_children, old_values, old_next) = {
                    let node = &mut self.nodes[*idx];
                    let keys = node.keys.drain(..).collect::<Vec<_>>();

                    match &mut node.node_type {
                        NodeType::Branch { children } => {
                            let old_children = children.drain(..).collect::<Vec<_>>();
                            (keys, Some(old_children), None, None)
                        },
                        NodeType::Leaf { values, next } => {
                            let old_values = values.drain(..).collect::<Vec<_>>();
                            (keys, None, Some(old_values), Some(*next))
                        },
                    }
                };

                let parent = &self.nodes[parent_idx];
                let sep_key = parent.keys[pos - 1].clone();
                let merge_node = &mut self.nodes[sib_idx];

                match &mut merge_node.node_type {
                    NodeType::Leaf { values, next } => {
                        if let Some(old_values) = old_values {
                            values.extend(old_values);
                        }
                        *next = old_next.unwrap();
                    },
                    NodeType::Branch { children } => {
                        if let Some(old_children) = old_children {
                            merge_node.keys.push(sep_key);
                            children.extend(old_children);
                        }
                    },
                }
                merge_node.keys.extend(old_keys);

                let parent = &mut self.nodes[parent_idx];
                parent.keys.remove(pos - 1);
                if let NodeType::Branch { children } = &mut parent.node_type {
                    children.remove(pos);
                }
            }

        }
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
        assert_eq!(Value::Int(1), tree.get("one").unwrap(), "Failure in first insertion");
        tree.insert("two", Value::Int(2));
        tree.print_tree();
        assert_eq!(Value::Int(2), tree.get("two").unwrap(), "Failure in second insertion");
        tree.insert("three", Value::Int(3));
        tree.print_tree();
        assert_eq!(Value::Int(3), tree.get("three").unwrap(), "Failure in third insertion");
        tree.insert("four", Value::Int(4));
        tree.print_tree();
        assert_eq!(Value::Int(4), tree.get("four").unwrap(), "Failure in fourth insertion");
        tree.insert("five", Value::Int(5));
        tree.print_tree();
        assert_eq!(Value::Int(5), tree.get("five").unwrap(), "Failure in fifth insertion");
        tree.insert("six", Value::Int(6));
        tree.print_tree();
        assert_eq!(Value::Int(6), tree.get("six").unwrap(), "Failure in sixth insertion");
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }

    #[test]
    fn stress_test() {
        for order in [3, 4, 5, 10] {
            for n in [10, 20, 50, 100] {
                let mut tree = BpTree::new(order);
                for i in 0..n {
                    tree.insert(&format!("key{:03}", i), Value::Int(i));
                    assert!(tree.validate().is_ok());
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
    fn show_tree() {
        let tree = build_tree(4, 16);
        tree.print_tree();
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }

    #[test]
    fn bptree_remove_simple() {
        let mut tree = build_tree(3, 20);
        tree.remove("key018");
        assert!(tree.get("key018").is_none());
        tree.print_tree();
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }

    #[test]
    fn bptree_remove_borrow() {
        let mut tree = build_tree(3, 20);
        tree.remove("key020");
        assert!(tree.get("key020").is_none());
        tree.print_tree();
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }
    
    #[test]
    fn bptree_remove_cascade() {
        let mut tree = build_tree(3, 20);
        tree.remove("key020");
        assert!(tree.get("key020").is_none());
        tree.remove("key019");
        assert!(tree.get("key019").is_none());
        tree.print_tree();
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }

    #[test]
    fn bptree_remove_merge() {
        let mut tree = build_tree(3, 20);
        tree.remove("key015");
        assert!(tree.get("key003").is_none());
        tree.remove("key014");
        assert!(tree.get("key006").is_none());
        tree.remove("key009");
        assert!(tree.get("key009").is_none());
        tree.remove("key012");
        assert!(tree.get("key012").is_none());
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
                if key >= next_key {
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

        // Ensure the key count is within the tree's defined order
        if idx != self.root && (node.keys.len() < (self.order + 1) / 2 ||
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
