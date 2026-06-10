pub mod value;
pub mod bptree;
pub mod pager;

use crate::store::bptree::BpTree;
use crate::store::pager::Pager;
use crate::store::value::Value;
use crate::logs::DbError;

pub const PAGE_SIZE: usize = 4096;
pub const DEFAULT_ORDER: usize = 256;

// Buffer pool for database, holds cache?
pub struct Store {
    datamap: BpTree,
    pager: Pager,
}

impl Store {
    pub fn start(filepath: &str) -> Result<Self, DbError> {
        let datamap = BpTree::new(DEFAULT_ORDER)?;
        let pager = Pager::open(filepath)?;
        Ok(Store {
            datamap: datamap,
            pager: pager,
        })
    }

    pub fn get(&self, key: &str) -> Result<Value, DbError> {
        match self.datamap.get(key, &self.pager) {
            Some(x) => Ok(x.clone()),
            None => Err(DbError::NoValue),
        }
    }

    pub fn put(&mut self, key: &str, val: Value) -> Result<Value, DbError> {
        let if_new = val.clone();
        match self.datamap.insert(key, val, &mut self.pager) {
            Some(x) => Ok(x),
            None => Ok(if_new),
        }
    }

    pub fn del(&mut self, key: &str) -> Result<Value, DbError> {
        match self.datamap.remove(key, &mut self.pager) {
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
        let mut db = Store::build();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn insert_and_remove() {
        let mut db = Store::build();
        let put = db.put("key", Value::Text("test".to_string()));
        assert_eq!(put.unwrap(), db.get("key").unwrap());
    }
}
