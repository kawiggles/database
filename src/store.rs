use std::collections::{HashMap};
use std::fmt;

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
    Exit,
}

impl Store {
    pub fn build() -> Self {
        Store {
            datamap: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Result<&Value, DbError> {
        self.datamap.get(key).ok_or(DbError::NoKey)?;
    }

    pub fn put(&mut self, key: &str, val: Value) {
        self.datamap.insert(key.to_owned(), val);
    }

    pub fn del(&mut self, key: &str) {
        self.datamap.remove(key);
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

// TODO: unify error handling 
#[derive(Debug)]
pub enum DbError {
    NoKey,
    NoValue,
    BadVal,
    BadCall,
}

impl From<DbError> for fmt::Error {
    fn from(_error: DbError) -> Self {
        fmt::Error
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(match self {
            DbError::NoKey => "No key found",
            DbError::NoValue => "No value found at requested key",
            DbError::BadVal => "Value input is invalid",
            DbError::BadCall => "API call is malformed",
        }))
    }
}
