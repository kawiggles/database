use std::io;
use log::info;

use crate::store::{Store, Value};
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
    Exit,
}

impl Call {
    pub fn parse(input: &str) -> Result<Self, DbError> {
        let args: Vec<&str> = input.trim().split_whitespace().collect();
        match args.as_slice() {
            ["get", key] => {
                info!("Input parsed as get");
                Ok(Call::Get(key.to_string()))
            },
            ["put", key, val] => {
                info!("Input parsed as basic put");
                Ok(Call::Put { key: key.to_string(), value: Value::Text(val.to_string()) })
            }
            ["put", key, val, type_tag] => {
                info!("Input parsed as put");
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
            ["del", key] => Ok(Call::Del(key.to_string())),
            ["exit"] => Ok(Call::Exit),
            _ => Err(DbError::BadCall),
        }
    }

    pub fn execute(self, db: &mut Store) -> Result<Value, DbError> {
        info!("Executing call");
        match self {
            Call::Get(key) => db.get(&key),
            Call::Put { key, value } => {
                db.put(&key, value)
            },
            Call::Del(key) => {
                db.del(&key)
            },
            Call::Exit => Ok(Value::Null)
        }
    }
}
