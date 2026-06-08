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
    order: usize, // track branching factor
}

// TODO: Implement get, insert, and remove functions for BpTree
impl BpTree {
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

        // First we find the leaf node
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

        // Then we insert into that node
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

        // Finally we handle splits as necessary
        let mut path_iter = path.iter().rev().peekable();
        while let Some(index) = path_iter.next() {
            let split_result = {
                let nodes_len = self.nodes.len();
                let node = &mut self.nodes[*index];

                if node.keys.len() > self.order - 1 {
                    let mid = node.keys.len() / 2;
                    let new_keys = node.keys.split_off(mid);

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

    pub fn remove(&mut self, _key: &str) -> Option<&Value> {
        None
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
    fn test_tree() {
        let mut tree = BpTree::new(4);
        tree.insert("one", Value::Int(1));
        tree.insert("two", Value::Int(2));
        tree.insert("three", Value::Int(3));
        tree.insert("four", Value::Int(4));
        tree.insert("five", Value::Int(5));
        tree.insert("six", Value::Int(6));
        tree.insert("seven", Value::Int(7));
        tree.insert("eight", Value::Int(8));
        tree.insert("nine", Value::Int(9));
        tree.insert("ten", Value::Int(10));
        tree.print_tree();
        let result = tree.validate();
        assert!(result.is_ok(), "Error is: {:?}", result);
    }

    #[test]
    fn bptree_remove() {
        let mut tree = BpTree::new(2);
        let _ = tree.insert("one", Value::Int(1));
        let _ = tree.insert("two", Value::Int(2));
        let _ = tree.insert("three", Value::Int(3));
        let _ = tree.insert("four", Value::Int(4));
        let _ = tree.insert("five", Value::Int(5));
        let _ = tree.insert("six", Value::Int(6));
    }

}

#[cfg(test)]
impl BpTree {
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

    fn validate(&self) -> Result<(), TreeErr> {
        if self.nodes.is_empty() {
            return Err(TreeErr::Empty);
        }

        if let NodeType::Branch { children } = &self.nodes[self.root].node_type {
            if children.len() < 2 {
                return Err(TreeErr::RootTooFewChildren);
            }
        }

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

        return self.validate_node(self.root, 0, leaf_depth); 
    }

    fn validate_node(&self, idx: usize, depth: usize, leaf_depth: usize) -> Result<(), TreeErr> {
        let node = &self.nodes[idx];
        
        let mut iter = node.keys.iter().peekable();
        while let Some(key) = iter.next() {
            if let Some(next_key) = iter.peek() {
                if key >= next_key {
                    return Err(TreeErr::NodeKeySeqErr);
                }
            }
        }

        match &node.node_type {
            NodeType::Branch { children } => {
                if children.len() != node.keys.len() + 1 {
                    return Err(TreeErr::BranchChildCountErr);
                }
                if idx != self.root && (node.keys.len() < self.order / 2 ||
                    node.keys.len() > self.order - 1) {
                    return Err(TreeErr::BranchKeyCountErr);
                }
                for &child in children {
                    return self.validate_node(child, depth + 1, leaf_depth);
                }
            },
            NodeType::Leaf { values, .. } => {
                if depth != leaf_depth {
                    return Err(TreeErr::LeafBadDepth);
                }

                if values.len() != node.keys.len() {
                    return Err(TreeErr::KeyValueDesync);
                }

                if idx != self.root && (node.keys.len() < self.order / 2 ||
                    node.keys.len() > self.order - 1) {
                    return Err(TreeErr::LeafKeyCountErr);
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
    BranchChildCountErr,
    BranchKeyCountErr,
    LeafKeyCountErr,
    KeyValueDesync,
    LeafBadDepth,
}

#[cfg(test)]
impl From<TreeErr> for std::fmt::Error {
    fn from(_error: TreeErr) -> Self {
        std::fmt::Error
    }
}
