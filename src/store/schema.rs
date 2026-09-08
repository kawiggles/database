use std::{
    io::Read,
    str::from_utf8,
};

use crate::{
    errors::{StoreResult},
    store::{
        value::Value,
    },
};

#[derive(Copy, Clone)]
pub enum Type {
    Bool,
    Int,
    Uint,
    Float,
    Text,
    Blob
}

pub struct Column {
    pub name: String,
    pub ty: Type,
}

pub struct Schema(Vec<Column>);

impl Schema {
    pub fn from_static(cols: &[(&str, Type)]) -> Self {
        Self(cols.iter().map(|&(name, ty)| Column { name: name.into(), ty }).collect())
    }

    // TODO: bitmap for null values (for each schema col, there will be a bit in front that is
    // either 1 for read the value, or 0 for null). We'll return an optional for that
    pub fn to_vals(&self, mut row_bytes: &[u8]) -> StoreResult<Vec<Value>> {
        let mut vals: Vec<Value> = Vec::new();

        for col in &self.0 {
            match col.ty {
                Type::Bool => {
                    let mut buf = [0u8; 1];
                    row_bytes.read_exact(&mut buf)?;
                    // TODO: this is terrible, add an error
                    let val = if u8::from_le_bytes(buf) == 1 { true } else { false };
                    vals.push(Value::Bool(val));
                },
                Type::Int => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = isize::from_le_bytes(buf);
                    vals.push(Value::Int(val));
                },
                Type::Uint => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = usize::from_le_bytes(buf);
                    vals.push(Value::Uint(val));
                },
                Type::Float => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = f64::from_le_bytes(buf);
                    vals.push(Value::Float(val));
                },
                Type::Text => {
                    let mut buf = [0u8; 2];
                    row_bytes.read_exact(&mut buf)?;
                    let len = u16::from_le_bytes(buf);
                    let mut buf: Vec<u8> = vec![0; len as usize];
                    row_bytes.read_exact(&mut buf)?;
                    let val: String = from_utf8(&buf)?.into();
                    vals.push(Value::Text(val));
                },
                Type::Blob => {
                    let mut buf = [0u8; 2];
                    row_bytes.read_exact(&mut buf)?;
                    let len = u16::from_le_bytes(buf);
                    let mut buf: Vec<u8> = vec![0; len as usize];
                    row_bytes.read_exact(&mut buf)?;
                    vals.push(Value::Blob(buf));
                },
            }
        }

        Ok(vals)
    }

    // this will probably replace the whole value to_le_bytes thing, but maybe not
    pub fn to_bytes(&self, vals: Vec<Value>) -> StoreResult<()> {
        todo!()
    }
}
