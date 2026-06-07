use std::collections::{HashMap};
use std::fmt;

pub struct Store {
    // TODO: Turn this into a B+ tree with indexes instead of reference
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
    // Not sure if this is necessary yet, check Call::execute for current usage()
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

#[derive(Debug)]
pub enum DbError {
    NoKey,
    NoValue,
    BadVal,
    BadCall,
    BadPut,
    BadDel,
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
            DbError::BadPut => "Put call was unsuccessful",
            DbError::BadDel => "Del call was unsuccessful",
        }))
    }
}
