use crate::store::value::Value;
use crate::logs::{Result, DbError};
use log::{info};

#[derive(Debug, PartialEq)]
pub enum Query {
    Get(String),
    Put {
        key: String,
        value: Value,
    },
    Del(String),
    Info, // Could also have this be a string
}

impl Query {
    // This will eventually get replaced with sql parser
    pub fn parse(input: &str) -> Result<Self> {
        let args: Vec<&str> = input.trim().split_whitespace().collect();
        match args.as_slice() {
            ["GET", key] => {
                info!("Input parsed as GET");
                Ok(Query::Get(key.to_string()))
            },
            ["SET", key, val] => {
                info!("Input parsed as SET");
                Ok(Query::Put { key: key.to_string(), value: Value::Text(val.to_string()) })
            },
            ["SET", key, val, type_tag] => {
                info!("Input parsed as typed SET");
                let value: Option::<Value> = match *type_tag {
                    "int" => Some(Value::Int(val.parse::<isize>().map_err(|_| DbError::BadVal)?)),
                    "float" => Some(Value::Float(val.parse::<f64>().map_err(|_| DbError::BadVal)?)),
                    "text" => Some(Value::Text(val.to_string())),
                    "blob" => {
                        let bytes: Result<Vec<u8>> = val
                            .trim_matches(&['[', ']'])
                            .split(',')
                            .map(|s| s.parse::<u8>().map_err(|_| DbError::BadVal))
                            .collect();
                        Some(Value::Blob(bytes?))
                    }
                    _ => None
                };
                match value {
                    Some(x) => Ok(Query::Put { key: key.to_string(), value: x }),
                    None => Err(DbError::BadVal),
                }
            },
            ["DEL", key] => Ok(Query::Del(key.to_string())),
            _ => Err(DbError::BadCall), // TODO: rename to BadQuery
        }
    }
}
