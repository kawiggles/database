#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(isize),
    Uint(usize),
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
            Value::Bool(x) => if *x { "true".into() } else { "false".into() },
            Value::Int(x) => x.to_string(),
            Value::Uint(x) => x.to_string(),
            Value::Float(x) => x.to_string(),
            Value::Text(x) => x.to_string(),
            Value::Blob(x) => format!("[{}]", x.iter()
                .map(|byte| byte.to_string())
                .collect::<Vec<String>>()
                .join(",")),
        }
    }

    // TODO: figure out whether or not I also need to do le bytes, and if blobs change
    // Might use this only for the wire protocol because that needs be bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        match self {
            Value::Bool(x) => if *x { buf.extend([1u8; 1]) } else { buf.extend([0u8; 1]) },
            Value::Int(x) => buf.extend(x.to_be_bytes()),
            Value::Uint(x) => buf.extend(x.to_be_bytes()),
            Value::Float(x) => buf.extend(x.to_be_bytes()),
            // TODO: Reminder for these two: you need to encode length as a u16 before encoding
            Value::Text(x) => buf.extend(x.as_bytes()),
            Value::Blob(x) => buf.extend(x),
        }
        buf
    }

    // Might not need this
    pub fn from_bytes(bytes: &[u8]) -> Self {
        todo!()
    }
}
