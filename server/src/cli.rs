use std::io;
use log::info;

use crate::store::{Store, value::Value};
use crate::logs::DbError;

pub fn get_input() -> String {
    println!("Enter an API call: ");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    info!("Input entered: {}", input);
    input
}

#[derive(Debug, PartialEq)]
pub enum Call {
    Get(String),
    Put {
        key: String,
        value: Value,
    },
    Del(String),
    Help,
    Exit,
}

impl Call {
    pub fn parse(input: &str) -> Result<Self, DbError> {
        let args: Vec<&str> = input.trim().split_whitespace().collect();
        match args.as_slice() {
            ["GET", key] => {
                info!("Input parsed as SET");
                Ok(Call::Get(key.to_string()))
            },
            ["SET", key, val] => {
                info!("Input parsed as basic SET");
                Ok(Call::Put { key: key.to_string(), value: Value::Text(val.to_string()) })
            }
            ["SET", key, val, type_tag] => {
                info!("Input parsed as SET");
                let value: Option::<Value> = match *type_tag {
                    "int" => Some(Value::Int(val.parse::<i64>().map_err(|_| DbError::BadVal)?)),
                    "float" => Some(Value::Float(val.parse::<f64>().map_err(|_| DbError::BadVal)?)),
                    "text" => Some(Value::Text(val.to_string())),
                    "blob" => {
                        let bytes: Result<Vec<u8>, DbError> = val
                            .trim_matches(&['[', ']'])
                            .split(',')
                            .map(|s| s.parse::<u8>().map_err(|_| DbError::BadVal))
                            .collect();
                        Some(Value::Blob(bytes?))
                    }
                    _ => None
                };
                match value {
                    Some(x) => Ok(Call::Put { key: key.to_string(), value: x }),
                    None => Err(DbError::BadVal),
                }
            }
            ["DEL", key] => Ok(Call::Del(key.to_string())),
            ["HELP"] => Ok(Call::Help),
            ["EXIT"] => Ok(Call::Exit),
            _ => Err(DbError::BadCall),
        }
    }

    pub fn execute(&self, db: &mut Store) -> Result<Value, DbError> {
        info!("Executing call");
        match self {
            Call::Get(key) => db.get(&key),
            Call::Put { key, value } => {
                db.put(&key, value.to_owned())
            },
            Call::Del(key) => {
                db.del(&key)
            },
            Call::Help => {
                info!("Help call parsed");
                println!("<call> key value value_type");
                println!("Valid calls: GET, SET, DEL");
                println!("Valid value types: text, int, float, blob");
                println!(" - Format blobs with as [1,2,3]");
                println!(" - put defaults to value_type text");
                Ok(Value::Null)
            },
            Call::Exit => {
                db.exit()?;
                Ok(Value::Null)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn cli_parse_get() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path).unwrap();
        let _ = db.put("key", Value::Text(String::from("test")));
        let test: Call = Call::parse("GET key").unwrap();
        assert_eq!(test.execute(&mut db).unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn cli_parse_put() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path).unwrap();
        let test: Call = Call::parse("SET key value").unwrap();
        assert_eq!(test.execute(&mut db).unwrap(), db.get("key").unwrap());
    }

    #[test]
    fn cli_parse_del() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut db = Store::start(path).unwrap();
        let _ = db.put("key", Value::Int(3));
        let test: Call = Call::parse("DEL key").unwrap();
        assert_eq!(Value::Int(3), test.execute(&mut db).unwrap());
        assert!(db.get("key").is_err());
    }
}
