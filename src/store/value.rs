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
    pub fn bool(b: bool) -> Self { Value::Bool(b) }
    pub fn int(n: isize) -> Self { Value::Int(n) }
    pub fn uint(n: usize) -> Self { Value::Uint(n) }
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
}
