use crate::logs::DbError;
use crate::store::value::Value;

pub enum NodeType {
    Branch { children: Vec<usize> },
    Leaf { values: Vec<Value>, next: Option<usize> },
}

pub struct Node {
    keys: Vec<String>,
    node_type: NodeType,
}

impl Node {
    // might be able to make these private after testing
    fn new_leaf() -> Self {
        Node {
            keys: Vec::new(),
            node_type: NodeType::Leaf { 
                values: Vec::new(), 
                next: None,
            }
        }
    }

    fn new_branch() -> Self {
        Node {
            keys: Vec::new(),
            node_type: NodeType::Branch { 
                children: Vec::new(),
            }
        }
    }
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

    pub fn get(&self, key: &str) -> Result<Value, DbError> {
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
                NodeType::Leaf { values, next: _ } => {
                    return match node.keys.binary_search_by(|probe| probe.as_str().cmp(key)) {
                        Ok(i) => Ok(values[i].clone()),
                        Err(_) => Err(DbError::NoValue),
                    }
                }
            }
        }
    }

    pub fn insert(&mut self, key: &str) -> Option<&Value> {
        None
    }

    pub fn remove(&mut self, key: &str) -> Option<&Value> {
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
    }
}
