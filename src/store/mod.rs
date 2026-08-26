pub mod value;
pub mod bptree;
pub mod pager;

use std::fs;
use log::{info, warn};

use crate::{
    store::{
        bptree::BpTree, 
        pager::{ Pager, BranchPage, LeafPage, DataPage, Page, PageId, PageType },
        value::Value,
    },
    errors::{ UserErr, DbResult, StoreResult, StoreErr, TreeErr},
};

pub const PAGE_SIZE: usize = 4096;
pub const DEFAULT_ORDER: usize = 150; // Back of the napkin math got me here

pub const RID_SIZE: usize = 16; // usize + usize when serialized
#[derive(Clone, Copy)]
pub struct Rid {
    pub page: PageId,
    pub slot: usize,
}

// Buffer pool for database, holds cache?
pub struct Store {
    pub tree: BpTree,
    pub pager: Pager,
}

impl Store {
    pub fn start(filepath: &str, new_order: usize) -> DbResult<Self> {
        let is_initialized = fs::metadata(filepath)
            .map(|m| m.len() >= PAGE_SIZE as u64)
            .unwrap_or(false);

        let (pager, root, order) = if is_initialized {
            info!(" - Opening existing database at {}", filepath);
            Pager::open(filepath)?
        } else {
            warn!(" - Database not found at path, creating new database at {}", filepath);
            Pager::new(filepath, new_order)?
        };
        
        let tree = BpTree::new(root, order);
        Ok(Store {
            tree: tree,
            pager: pager,
        })
    }

    pub fn get(&mut self, key: &str) -> DbResult<Value> {
        let rid = self.tree.get(key, &mut self.pager)?;
        let mut page = self.pager.read::<DataPage>(rid.id)?;
        Ok(page.get_slot(rid.slot)?)
    }

    pub fn put(&mut self, key: &str, val: Value) -> DbResult<Value> {
        // TODO: Eliminate value limits
        // This is in place because of how keys are encoded by bincode, see pager
        if key.len() > 8 {
            return Err(UserErr::LongKey)?
        }

        // TODO: Value Overflow logic

        let if_new = val.clone();
        match self.tree.insert(key, val, &mut self.pager)? {
            Some(x) => Ok(x),
            None => Ok(if_new),
        }
    }

    pub fn del(&mut self, key: &str) -> DbResult<Value> {
        self.tree.remove(key, &mut self.pager)
    }

    pub fn exit(&mut self) -> DbResult<()> {
        self.pager.close(self.tree.root, self.tree.order)?;
        Ok(())
    }

    // Assorted functions for displaying information in tests
    fn print_page(&mut self, page_id: PageId, prefix: &str, is_last: bool) -> StoreResult<()> {
        print!("{}", prefix);
        if is_last {
            print!("└── ");
        } else {
            print!("├── ");
        }
        
        let header = self.pager.read_header(page_id)?;
        match header.pagetype {
            PageType::Leaf => {
                let next_str = match header.next {
                    Some(idx) => format!(" -> (id: {:?})", idx),
                    None => " -> []".to_string(),
                };
                let page = self.pager.read::<LeafPage>(page_id)?;
                println!("Leaf(id: {:?}, keys: {:?}){}", page_id, page.keys, next_str);
                Ok(())
            },
            PageType::Branch => {
                let page = self.pager.read::<BranchPage>(page_id)?;
                println!("Branch(id: {:?}, keys: {:?})", page_id, page.keys);
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else {"|   "});
                for (i, &child_idx) in page.children.iter().enumerate() {
                    let child_is_last = i == page.children.len() - 1;
                    self.print_page(child_idx, &new_prefix, child_is_last);
                }
                Ok(())
            },
            _ => Err(StoreErr::UnexpectedPagetype{
                found: header.pagetype,
                expected: PageType::Branch,
            })
        }
    }

    pub fn print_tree(&mut self) {
        println!();
        let Some(root) = self.tree.root else {
            println!("Tree is empty");
            println!();
            return;
        };

        println!("Root (id: {:?})", root);
        self.print_page(root, "", true);
        println!();
    }

    // Function to check if a tree is valid
    pub fn validate(&self) -> StoreResult<()> {
        let Some(root) = self.tree.root else {
            return Err(StoreErr::TreeErr(TreeErr::Empty));
        };

        let header = self.pager.read_header(root)?;
        if let PageType::Branch = header.pagetype {
            let page = self.pager.read::<BranchPage>(root)?;
            if page.children.len() < 2 {
                return Err(StoreErr::TreeErr(TreeErr::RootTooFewChildren));
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
        if let Some(root) = self.tree.root {
            if idx != root && (current.keys.len() < self.tree.order / 2 ||
                current.keys.len() > self.tree.order - 1) {
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
