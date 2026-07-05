pub mod value;
pub mod bptree;
pub mod pager;

use std::fs;
use log::{info, warn};

use crate::store::bptree::BpTree;
use crate::store::value::Value;
use crate::store::pager::{Pager, NodeType, IndexPage, PageId, Page};
use crate::logs::{DbErr, UserErr};

pub const PAGE_SIZE: usize = 4096;
pub const DEFAULT_ORDER: usize = 150; // Back of the napkin math got me here

// Buffer pool for database, holds cache?
pub struct Store {
    pub datamap: BpTree,
    pub pager: Pager,
}

impl Store {
    pub fn start(filepath: &str, new_order: usize) -> Result<Self, DbErr> {
        let is_initialized = fs::metadata(filepath)
            .map(|m| m.len() >= PAGE_SIZE as u64)
            .unwrap_or(false);

        let (pager, root, order) = if is_initialized {
            info!("Opening existing database at {}", filepath);
            Pager::open(filepath)?
        } else {
            warn!("Database not found at path, creating new database at {}", filepath);
            Pager::new(filepath, new_order)?
        };
        
        let datamap = BpTree::new(root, order);
        Ok(Store {
            datamap: datamap,
            pager: pager,
        })
    }

    pub fn get(&self, key: &str) -> Result<Value, DbErr> {
        self.datamap.get(key, &self.pager)
    }

    pub fn put(&mut self, key: &str, val: Value) -> Result<Value, DbErr> {
        // TODO: Eliminate value limits
        // This is in place because of how keys are encoded by bincode, see pager
        if key.len() > 8 {
            return Err(UserErr::LongKey)?
        }

        // TODO: Value Overflow logic

        let if_new = val.clone();
        match self.datamap.insert(key, val, &mut self.pager)? {
            Some(x) => Ok(x),
            None => Ok(if_new),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<Value, DbErr> {
        self.datamap.remove(key, &mut self.pager)
    }

    pub fn exit(&mut self) -> Result<(), DbErr> {
        self.pager.close(self.datamap.root, self.datamap.order)?;
        Ok(())
    }

    // Assorted functions for displaying information in tests
    fn print_page(&self, page_id: PageId, prefix: &str, is_last: bool) {
        let page: IndexPage = Page::read(&self.pager, page_id).unwrap();

        print!("{}", prefix);
        if is_last {
            print!("└── ");
        } else {
            print!("├── ");
        }
        
        match &page.node_type {
            NodeType::Leaf { pages: _ , next } => {
                let next_str = match next {
                    Some(idx) => format!(" -> [{}]", idx),
                    None => " -> []".to_string(),
                };
                println!("[Leaf: {}] keys: {:?}{}", page_id, page.keys, next_str);
            },
            NodeType::Branch { children } => {
                println!("[Branch: {}] keys: {:?}", page_id, page.keys);
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else {"|   "});
                for (i, &child_idx) in children.iter().enumerate() {
                    let child_is_last = i == children.len() - 1;
                    self.print_page(child_idx, &new_prefix, child_is_last);
                }
            },
        }
    }

    pub fn print_tree(&self) {
        println!();
        let Some(root) = self.datamap.root else {
            println!("Tree is empty");
            println!();
            return;
        };

        println!("Tree Structure: (Root: {})", root);
        self.print_page(root, "", true);
        println!();
    }

    // Function to check if a tree is valid
    pub fn validate(&self) -> Option<TreeErr> {
        let Some(root) = self.datamap.root else {
            return Some(TreeErr::Empty);
        };

        let root_page: IndexPage = Page::read(&self.pager, root).unwrap();
        if let NodeType::Branch { children } = root_page.node_type {
            if children.len() < 2 {
                return Some(TreeErr::RootTooFewChildren);
            }
        }

        // Get leaf depth and traverse to leftmost leaf
        let mut leaf_depth = 0;
        let mut current = root;
        loop {
            let page: IndexPage = Page::read(&self.pager, current).unwrap();
            match page.node_type {
                NodeType::Branch { children } => {
                    leaf_depth += 1;
                    current = children[0];
                },
                NodeType::Leaf { .. } => break,
            }
        }

        // Check all keys are ordered and that no branches are leaf level
        let mut prev_key: Option<String> = None;
        loop {
            let page: IndexPage = Page::read(&self.pager, current).unwrap();
            match  page.node_type {
                NodeType::Leaf { next, .. } => {
                    for key in page.keys {
                        if let Some(prev) = prev_key {
                            if key <= prev {
                                return Some(TreeErr::LeafKeysBadSeq);
                            }
                        }
                        prev_key = Some(key);
                    }

                    match next {
                        Some(x) => current = x,
                        None => break,
                    }
                },
                NodeType::Branch { .. } => return Some(TreeErr::BranchInLeafSeq),
            }
        }
        
        // Recurse through each node checking leaves
        return self.validate_page(root, 0, leaf_depth, None, None); 
    }

    fn validate_page(&self, idx: PageId, depth: usize, leaf_depth: usize, 
        min_bound: Option<&str>, max_bound: Option<&str>)
        -> Option<TreeErr> {
        
        let current: IndexPage = Page::read(&self.pager, idx).unwrap();
        
        // Ensure keys are properly ordered
        let mut iter = current.keys.iter().peekable();
        while let Some(key) = iter.next() {
            if let Some(next_key) = iter.peek() {
                if key >= *next_key {
                    return Some(TreeErr::NodeKeySeqErr(idx));
                }
            }
        }

        // Check all keys are in the bounds defined by the parent node
        for key in &current.keys {
            if let Some(min) = min_bound {
                if key.as_str() <= min { return Some(TreeErr::KeyOOB(idx)); }
            }
            if let Some(max) = max_bound {
                if key.as_str() > max { return Some(TreeErr::KeyOOB(idx)); }
            }
        }

        // Ensure node is above minimum value
        if let Some(root) = self.datamap.root {
            if idx != root && (current.keys.len() < self.datamap.order / 2 ||
                current.keys.len() > self.datamap.order - 1) {
                return Some(TreeErr::KeyCountErr(idx));
            }
        }

        match &current.node_type {
            NodeType::Branch { children } => {
                // Each branch should have keys + 1 children
                if children.len() != current.keys.len() + 1 {
                    return Some(TreeErr::KeyChildDesync(idx));
                }

                for (i, &child) in children.iter().enumerate() {
                    // Set the new bounds and check children
                    let new_min = if i > 0 { 
                        Some(current.keys[i-1].as_str()) 
                    } else { min_bound };
                    let new_max = if i < current.keys.len() { 
                        Some(current.keys[i].as_str()) 
                    } else { max_bound };
                    return self.validate_page(child, depth + 1, leaf_depth, new_min, new_max);
                }
            },
            NodeType::Leaf { pages, .. } => {
                // Make sure leaf is at the proper depth
                if depth != leaf_depth {
                    return Some(TreeErr::LeafBadDepth(idx));
                }

                // Ensure leaves have the same number of keys and values
                if pages.len() != current.keys.len() {
                    return Some(TreeErr::KeyValueDesync(idx));
                }

            },
        }
        None
    }
}

#[derive(Debug)]
pub enum TreeErr {
    Empty,
    RootTooFewChildren,
    LeafKeysBadSeq,
    BranchInLeafSeq,
    NodeKeySeqErr(PageId),
    KeyChildDesync(PageId),
    KeyCountErr(PageId),
    KeyValueDesync(PageId),
    LeafBadDepth(PageId),
    KeyOOB(PageId),
}

impl From<TreeErr> for std::fmt::Error {
    fn from(_error: TreeErr) -> Self {
        std::fmt::Error
    }
}

impl std::fmt::Display for TreeErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			TreeErr::Empty => write!(f, "Tree is empy"),
			TreeErr::RootTooFewChildren => write!(f, "Less than 2 children in root node"),
			TreeErr::LeafKeysBadSeq => write!(f, "Leaf node keys not sorted"),
			TreeErr::BranchInLeafSeq => write!(f, "Branch node found at leaf level"),
			TreeErr::NodeKeySeqErr(idx) => write!(f, "Page {} has unsorted keys", idx),
			TreeErr::KeyChildDesync(idx) => {
                write!(f, "Branch {} has wrong number of keys or children", idx)
            },
			TreeErr::KeyCountErr(idx) => {
                write!(f, "Node {} is outside min or max number of keys for order", idx)
            },
			TreeErr::KeyValueDesync(idx) => {
                write!(f, "Leaf {} has an unequal number of keys and values", idx)
            },
			TreeErr::LeafBadDepth(idx) => write!(f, "Leaf {} is not at leaf node depth", idx),
			TreeErr::KeyOOB(idx) => {
                write!(f, "Node {} has keys out of bounds defined by parent", idx)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn insert_and_get() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path, 4).unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path, 4).unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }
}
