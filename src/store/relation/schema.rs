use std::{
    io::Read,
    str::from_utf8,
};

use crate::{
    errors::{StoreResult, StoreErr},
    store::value::Value,
};

use super::{
    Row
};

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Type {
    Bool = 0,
    Int = 1,
    Uint = 2,
    Float = 3,
    Text = 4,
    Blob = 5,
}

impl TryFrom<usize> for Type {
    type Error = StoreErr;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
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

#[derive(Debug, PartialEq, Clone)]
pub struct Column {
    pub tid: usize,
    pub attnum: usize,
    pub name: String,
    pub ty: Type,
    pub is_key: bool,
    pub nullable: bool,
    pub is_dead: bool,
}

impl Column {
    pub fn attr_key(&self) -> String {
        format!("{:020}{:020}", self.tid, self.attnum)
    }
}

impl From<&Column> for Row {
    fn from(col: &Column) -> Self {
        Self(vec![
            Some(Value::Uint(col.attnum)),
            Some(Value::Uint(col.tid)),
            Some(Value::Text(col.name.clone())),
            Some(Value::Uint(col.ty as usize)),
            Some(Value::Bool(col.is_key)),
            Some(Value::Bool(col.nullable)),
            Some(Value::Bool(col.is_dead)),
        ])
    }
}

impl TryFrom<Row> for Column {
    type Error = StoreErr;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        Ok(Self {
            tid: row.field(0)?,
            attnum: row.field(1)?,
            name: row.field(2)?,
            ty: row.field(3)?,
            is_key: row.field(4)?,
            nullable: row.field(5)?,
            is_dead: row.field(6)?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Schema(pub Vec<Column>);

// TODO: for future optimization: use split on the bytestream instead of reading to reduce copying
impl Schema {

    /// Used to generate Schemas for the catalog's hardcoded tables
    pub fn from_static(cols: &[(&str, Type, bool)], tid: usize) -> Self {
        Self(cols.iter().enumerate().map(|(i, &(name, ty, is_key))| Column {
            tid,
            attnum: i,
            name: name.into(),
            ty,
            is_key,
            nullable: false,
            is_dead: false,
        }).collect())
    }

    /// Generates a key for a row using this schema
    pub fn gen_key(&self, row: &Row) -> StoreResult<String> {
        self.0.iter().enumerate()
            .filter(|(_, col)| col.is_key)
            .map(|(i, _)| row.0.get(i)
                .ok_or(StoreErr::BadColIndex(i))?
                .as_ref()
                .ok_or(StoreErr::NullField(i))?
                .as_key())
            .collect()
    }

    pub fn decode(&self, mut row_bytes: &[u8]) -> StoreResult<Row> {
        let mut vals: Vec<Option<Value>> = Vec::with_capacity(self.0.len());

        let mut bitmap = vec![0u8; self.0.len().div_ceil(8)];
        row_bytes.read_exact(&mut bitmap)?;

        for (i, col) in self.0.iter().enumerate() {
            if bm_nullable(&bitmap, i) {
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

        Ok(Row(vals))
    }

    // TODO: check values here instead of outside of function
    pub fn encode(&self, entries: Row) -> StoreResult<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        let mut bitmap = vec![0u8; self.0.len().div_ceil(8)];

        for (i, entry) in entries.0.iter().enumerate() {
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

fn bm_nullable(map: &[u8], col: usize) -> bool {
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
            ("bool", Type::Bool, false),
            ("int", Type::Int, true),
            ("uint", Type::Uint, false),
            ("float", Type::Float, false),
            ("text", Type::Text, false),
            ("blob", Type::Blob, false),
            ("null", Type::Text, false),
        ], 5);

        let vals: Row = Row(vec![
            Some(Value::Bool(true)),
            Some(Value::Int(-4)),
            Some(Value::Uint(4)),
            Some(Value::Float(5.5)),
            Some(Value::Text("this is text".into())),
            Some(Value::Blob(vec![b'h', b'i'])),
            None
        ]);

        let encoded = schema.encode(vals.clone()).unwrap();
        assert_eq!(vals, schema.decode(&encoded).unwrap());
    }
}
