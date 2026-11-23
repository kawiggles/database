use crate::{
    errors::{StoreErr, StoreResult},
    store::{
        relation::schema::Type,
        pager::page::PageId,
    },
};

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

    pub fn as_key(&self) -> StoreResult<String> {
        match self {
            Value::Uint(x) => Ok(format!("{:020}", x)),
            Value::Int(x) => Ok(format!("{:020}", (*x as usize) ^ ( 1 << (usize::BITS - 1)))),
            Value::Bool(x) => Ok(if *x { "1" } else { "0" }.into()),
            Value::Text(x) => Ok(x.clone()),
            Value::Float(_) | Value::Blob(_) => Err(StoreErr::NotKeyable),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        todo!()
    }

    pub fn ty(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::Text(_) => Type::Text,
            Value::Int(_) => Type::Int,
            Value::Uint(_) => Type::Uint,
            Value::Float(_) => Type::Float,
            Value::Blob(_) => Type::Blob,
        }
    }

    pub fn coerce(&self, ty: Type) -> StoreResult<Self> {
        match (self, ty) {
            (Value::Bool(b), Type::Int) => Ok(Value::Int((*b).into())),
            (Value::Bool(b), Type::Uint) => Ok(Value::Uint((*b).into())),
            (Value::Bool(b), Type::Float) => Ok(Value::Float((*b).into())),
            (Value::Bool(b), Type::Text) => {
                if *b { Ok(Value::Text("true".into())) } else { Ok(Value::Text("false".into())) }
            },
            (Value::Bool(b), Type::Blob) => {
                if *b { Ok(Value::Blob(vec![1])) } else { Ok(Value::Blob(vec![0])) }
            },
            // TODO: make an error type for failed coercions
            _ => todo!()
        }
    }
}


// TODO: turn all of these nearly identical functions into a macro_rules!
impl TryFrom<&Value> for bool {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Bool(x) => Ok(*x),
            _ => Err(StoreErr::TypeErr(Type::Bool)),
        }
    }
}

impl TryFrom<&Value> for isize {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Int(x) => Ok(*x),
            _ => Err(StoreErr::TypeErr(Type::Int)),
        }
    }
}

impl TryFrom<&Value> for usize {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Uint(x) => Ok(*x),
            _ => Err(StoreErr::TypeErr(Type::Uint)),
        }
    }
}

impl TryFrom<&Value> for PageId {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Uint(x) => Ok(PageId::new(*x).ok_or(StoreErr::PageIdZero)?),
            _ => Err(StoreErr::TypeErr(Type::Uint)),
        }
    }
}

impl TryFrom<&Value> for Type {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match usize::try_from(val)? {
            0 => Ok(Type::Bool),
            1 => Ok(Type::Int),
            2 => Ok(Type::Uint),
            3 => Ok(Type::Float),
            4 => Ok(Type::Text),
            5 => Ok(Type::Blob),
            _ => Err(StoreErr::UnexpectedType)
        }
    }
}

impl TryFrom<&Value> for f64 {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Float(x) => Ok(*x),
            _ => Err(StoreErr::TypeErr(Type::Float)),
        }
    }
}

impl TryFrom<&Value> for String {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Text(x) => Ok(x.clone()),
            _ => Err(StoreErr::TypeErr(Type::Text)),
        }
    }
}

impl TryFrom<&Value> for Vec<u8> {
    type Error = StoreErr;

    fn try_from(val: &Value) -> Result<Self, Self::Error> {
        match val {
            Value::Blob(x) => Ok(x.clone()),
            _ => Err(StoreErr::TypeErr(Type::Blob)),
        }
    }
}

impl From<bool> for Value {
    fn from(val: bool) -> Self {
        Self::Bool(val)
    }
}

impl From<isize> for Value {
    fn from(val: isize) -> Self {
        Self::Int(val)
    }
}

impl From<usize> for Value {
    fn from(val: usize) -> Self {
        Self::Uint(val)
    }
}

impl From<f64> for Value {
    fn from(val: f64) -> Self {
        Self::Float(val)
    }
}

impl From<String> for Value {
    fn from(val: String) -> Self {
        Self::Text(val)
    }
}

impl From<Vec<u8>> for Value {
    fn from(val: Vec<u8>) -> Self {
        Self::Blob(val)
    }
}
