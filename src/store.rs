use std::collections::{HashMap};

pub struct Store {
    datamap: HashMap<String, Value>
}

pub enum Call {
    Get(String),
    Put {
        key: String,
        value: Value,
    },
    Del(String),
}

impl Store {
    pub fn build() -> Self {
        Store {
            datamap: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Result<&Value, DbError> {
        self.datamap.get(key).ok_or(DbError::NoKey)
    }

    pub fn put(&mut self, key: &str, val: Value) {

    }

    pub fn del(&mut self, key: &str) {
    }
}

#[derive(Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
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
}

#[derive(Debug)]
pub enum DbError {
    NoKey,
    NoValue,
}
