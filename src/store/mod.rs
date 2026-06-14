pub mod value;
pub mod bptree;
pub mod pager;

use crate::store::bptree::BpTree;
use crate::store::value::Value;
use crate::store::pager::Pager;
use crate::logs::DbError;

use std::fs;

pub const PAGE_SIZE: usize = 4096;
pub const DEFAULT_ORDER: usize = 150; // Back of the napkin math got me here
pub const DEFAULT_FILE: &str = "kawika.db";

// Buffer pool for database, holds cache?
pub struct Store {
    datamap: BpTree,
    pager: Pager,
}

impl Store {
    pub fn start() -> Result<Self, DbError> {
        let (pager, root, order) = match fs::exists(DEFAULT_FILE) {
            Ok(true) => Pager::open(DEFAULT_FILE)?,
            Ok(false) => Pager::new(DEFAULT_FILE)?,
            Err(e) => return Err(DbError::IOErr(e)),
        };
        
        let datamap = BpTree::new(root, order);
        Ok(Store {
            datamap: datamap,
            pager: pager,
        })
    }

    pub fn get(&mut self, key: &str) -> Result<Value, DbError> {
        self.datamap.get(key, &mut self.pager)
    }

    pub fn put(&mut self, key: &str, val: Value) -> Result<Value, DbError> {
        if key.len() > 8 {
            return Err(DbError::LongKey)
        }

        // TODO: Value Overflow logic

        let if_new = val.clone();
        match self.datamap.insert(key, val)? {
            Some(x) => Ok(x),
            None => Ok(if_new),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<Value, DbError> {
        match self.datamap.remove(key) {
            Some(x) => Ok(x),
            None => Err(DbError::BadDel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut db = Store::start().unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let mut db = Store::start().unwrap();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }
}
