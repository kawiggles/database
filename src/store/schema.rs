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

// TODO: for future optimization: use split on the bytestream instead of reading to reduce copying
impl Schema {
    pub fn from_static(cols: &[(&str, Type)]) -> Self {
        Self(cols.iter().map(|&(name, ty)| Column { name: name.into(), ty }).collect())
    }

    pub fn decode(&self, mut row_bytes: &[u8]) -> StoreResult<Vec<Option<Value>>> {
        let mut vals: Vec<Option<Value>> = Vec::with_capacity(self.0.len());

        let mut bitmap = vec![0u8; self.0.len().div_ceil(8)];
        row_bytes.read_exact(&mut bitmap)?;

        for (i, col) in self.0.iter().enumerate() {
            if bm_is_null(&bitmap, i) {
                vals.push(None);
                continue;
            }

            match col.ty {
                Type::Bool => {
                    let mut buf = [0u8; 1];
                    row_bytes.read_exact(&mut buf)?;
                    // TODO: this is terrible, add an error
                    let val = if u8::from_le_bytes(buf) == 1 { true } else { false };
                    vals.push(Some(Value::Bool(val)));
                },
                Type::Int => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = isize::from_le_bytes(buf);
                    vals.push(Some(Value::Int(val)));
                },
                Type::Uint => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = usize::from_le_bytes(buf);
                    vals.push(Some(Value::Uint(val)));
                },
                Type::Float => {
                    let mut buf = [0u8; 8];
                    row_bytes.read_exact(&mut buf)?;
                    let val = f64::from_le_bytes(buf);
                    vals.push(Some(Value::Float(val)));
                },
                Type::Text => {
                    let mut buf = [0u8; 2];
                    row_bytes.read_exact(&mut buf)?;
                    let len = u16::from_le_bytes(buf);
                    let mut buf: Vec<u8> = vec![0; len as usize];
                    row_bytes.read_exact(&mut buf)?;
                    let val: String = from_utf8(&buf)?.into();
                    vals.push(Some(Value::Text(val)));
                },
                Type::Blob => {
                    let mut buf = [0u8; 2];
                    row_bytes.read_exact(&mut buf)?;
                    let len = u16::from_le_bytes(buf);
                    let mut buf: Vec<u8> = vec![0; len as usize];
                    row_bytes.read_exact(&mut buf)?;
                    vals.push(Some(Value::Blob(buf)));
                },
            }
        }

        Ok(vals)
    }

    pub fn encode(&self, entries: Vec<Option<Value>>) -> StoreResult<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        let mut bitmap = vec![0u8; self.0.len().div_ceil(8)];

        for (i, entry) in entries.iter().enumerate() {
            if let Some(val) = entry {
                match val {
                    Value::Bool(x) => bytes.push(*x as u8),
                    Value::Int(x) => bytes.extend_from_slice(&x.to_le_bytes()),
                    Value::Uint(x) => bytes.extend_from_slice(&x.to_le_bytes()),
                    Value::Float(x) => bytes.extend_from_slice(&x.to_le_bytes()),
                    Value::Text(x) => {
                        bytes.extend_from_slice(&(x.len() as u16).to_le_bytes());
                        bytes.extend_from_slice(x.as_bytes());
                    },
                    Value::Blob(x) => {
                        bytes.extend_from_slice(&(x.len() as u16).to_le_bytes());
                        bytes.extend_from_slice(&x);
                    },
                }
            } else {
                bm_set_null(&mut bitmap, i);
            }
        }

        Ok(bitmap.into_iter().chain(bytes).collect())
    }
}

fn bm_is_null(map: &[u8], col: usize) -> bool {
    map[col / 8] & (1 << (col % 8)) != 0
}

fn bm_set_null(map: &mut [u8], col: usize) {
    map[col / 8] |= 1 << (col % 8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_schema_roundtrip() {
        let schema = Schema::from_static(&[
            ("bool", Type::Bool),
            ("int", Type::Int),
            ("uint", Type::Uint),
            ("float", Type::Float),
            ("text", Type::Text),
            ("blob", Type::Blob),
            ("null", Type::Text),
        ]);

        let vals: Vec<Option<Value>> = vec![
            Some(Value::Bool(true)),
            Some(Value::Int(-4)),
            Some(Value::Uint(4)),
            Some(Value::Float(5.5)),
            Some(Value::Text("this is text".into())),
            Some(Value::Blob(vec![b'h', b'i'])),
            None
        ];

        let encoded = schema.encode(vals.clone()).unwrap();
        assert_eq!(vals, schema.decode(&encoded).unwrap());
    }
}
