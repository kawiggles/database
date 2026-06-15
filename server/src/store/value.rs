use bincode_next::{Encode, Decode};

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
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
    pub fn float(n: f64) -> Self { Value::Float(n) }
    pub fn text(s: &str) -> Self { Value::Text(s.to_owned()) }
    pub fn blob(b: Vec<u8>) -> Self { Value::Blob(b) }

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

// Implement this later
pub enum Key {
    Int(i64),
    Text(String),
}
