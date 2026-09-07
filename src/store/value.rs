#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(isize),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    pub fn int(n: isize) -> Self { Value::Int(n) }
    pub fn float(n: f64) -> Self { Value::Float(n) }
    pub fn text(s: &str) -> Self { Value::Text(s.to_owned()) }
    pub fn blob(b: Vec<u8>) -> Self { Value::Blob(b) }

    pub fn print(&self) -> String {
        match self {
            Value::Int(x) => x.to_string(),
            Value::Float(x) => x.to_string(),
            Value::Text(x) => x.to_string(),
            Value::Blob(x) => format!("[{}]", x.iter()
                .map(|byte| byte.to_string())
                .collect::<Vec<String>>()
                .join(",")),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        match self {
            Value::Int(x) => buf.extend(x.to_be_bytes()),
            Value::Float(x) => buf.extend(x.to_be_bytes()),
            Value::Text(x) => buf.extend(x.as_bytes()),
            Value::Blob(x) => buf.extend(x),
        }
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        todo!()
    }
}

// Implement this later
pub enum Key {
    Int(isize),
    Text(String),
}
