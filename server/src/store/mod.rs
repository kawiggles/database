pub mod value;
pub mod bptree;
pub mod pager;

use std::fs;
use log::{info, warn};

use crate::store::bptree::BpTree;
use crate::store::value::Value;
use crate::store::pager::Pager;
use crate::logs::{Result, DbError};

pub const PAGE_SIZE: usize = 4096;
pub const DEFAULT_ORDER: usize = 150; // Back of the napkin math got me here

// Buffer pool for database, holds cache?
pub struct Store {
    datamap: BpTree,
    pager: Pager,
}

impl Store {
    pub fn start(filepath: &str) -> Result<Self> {
        let is_initialized = fs::metadata(filepath)
            .map(|m| m.len() >= PAGE_SIZE as u64)
            .unwrap_or(false);

        let (pager, root, order) = if is_initialized {
            info!("Opening existing database at {}", filepath);
            Pager::open(filepath)?
        } else {
            warn!("Database not found at path, creating new database at {}", filepath);
            Pager::new(filepath)?
        };
        
        let datamap = BpTree::new(root, order);
        Ok(Store {
            datamap: datamap,
            pager: pager,
        })
    }

    pub fn get(&self, key: &str) -> Result<Value> {
        self.datamap.get(key, &self.pager)
    }

    pub fn put(&mut self, key: &str, val: Value) -> Result<Value> {
        if key.len() > 8 {
            return Err(DbError::LongKey)
        }

        // TODO: Value Overflow logic

        let if_new = val.clone();
        match self.datamap.insert(key, val, &mut self.pager)? {
            Some(x) => Ok(x),
            None => Ok(if_new),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<Value> {
        self.datamap.remove(key, &mut self.pager)
    }

    pub fn exit(&mut self) -> Result<()> {
        self.pager.close(self.datamap.root, self.datamap.order)?;
        Ok(())
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
        let mut db = Store::start(path).unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path).unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }
}
