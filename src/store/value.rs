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
