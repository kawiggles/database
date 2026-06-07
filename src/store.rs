use std::collections::{HashMap};
use crate::logs::DbError;

pub struct Node {
    keys: Vec<i64>,
    children: Vec<usize>, // usize is index of node in BpTree
    is_leaf: bool,
    next_leaf: Option<usize> // None if not leaf or end of leaf chain
}

pub struct BpTree {
    nodes: Vec<Node>,
    root: usize,
    order: usize, // track branching factor
}

// TODO: Implement get, insert, and remove functions for BpTree (or just use the store methods)
impl BpTree {
    pub fn new() -> Self {
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
    }

    pub fn insert(&mut self, key: &str) -> Option<&Value> {
    }

    pub fn remove(&mut self, key: &str) -> Option<&Value> {
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
    // Not sure if this is necessary yet, check Call::execute for current usage
    Null
}

impl Value {
    pub fn int(n: i64) -> Self { Value::Int(n) }
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(n) = self { Some(*n) } else { None } 
    }
    pub fn float(n: f64) -> Self { Value::Float(n) }
    pub fn as_float(&self) -> Option<f64> {
        if let Value::Float(n) = self { Some(*n) } else { None } 
    }
    pub fn text(s: &str) -> Self { Value::Text(s.to_owned()) }
    pub fn as_text(&self) -> Option<&str> { 
        if let Value::Text(s) = self { Some(s) } else { None } 
    }
    pub fn blob(b: Vec<u8>) -> Self { Value::Blob(b) }
    pub fn as_blob(&self) -> Option<&Vec<u8>> { 
        if let Value::Blob(b) = self { Some(b) } else { None } 
    }
    pub fn print(self) -> String {
        match self {
            Value::Int(x) => x.to_string(),
            Value::Float(x) => x.to_string(),
            Value::Text(x) => x,
            Value::Blob(x) => format!("[{}]", x.iter()
                .map(|byte| byte.to_string())
                .collect::<Vec<String>>()
                .join(",")),
            Value::Null => "null".to_string(),
        }
    }
}
