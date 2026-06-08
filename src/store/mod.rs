pub mod value;
pub mod bptree;

use std::collections::{HashMap};
use crate::store::value::Value;
use crate::logs::DbError;

// wrapper for basic data structure, with metadata (haven't figured that out yet)
pub struct Store {
    datamap: HashMap<String, Value>
}

impl Store {
    pub fn build() -> Self {
        Store {
            datamap: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Result<Value, DbError> {
        match self.datamap.get(key) {
            Some(x) => Ok(x.clone()),
            None => Err(DbError::NoValue),
        }
    }

    pub fn put(&mut self, key: &str, val: Value) -> Result<Value, DbError> {
        self.datamap.insert(key.to_owned(), val);
        self.get(key)
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
