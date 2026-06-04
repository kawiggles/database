use std::io;
use crate::store::{Call, Value};

// Temporary struct for error handling, need to refactor into singular error
#[derive(Debug)]
pub enum CLIErr {
    BadVal,
    BadCall,
}

pub fn get_input() -> Result<Call, CLIErr> {
    let mut input = String::new();

    io::stdin().read_line(&mut input).expect("Failed to read line");
    let args: Vec<&str> = input.trim().split_whitespace().collect();

    match args.as_slice() {
        ["get", key] => Ok(Call::Get(key.to_string())),
        ["put", key, type_tag, val] => {
            let value: Option::<Value> = match *type_tag {
                "int" => Some(Value::Int(val.parse::<i64>().ok().unwrap())),
                "float" => Some(Value::Float(val.parse::<f64>().ok().unwrap())),
                "text" => Some(Value::Text(val.to_string())),
                "blob" => Some(Value::Blob(val.trim_matches(&['[', ']'])
                    .split(',')
                    .into_iter()
                    .map(|s| s.parse::<u8>().ok().unwrap())
                    .collect())),
                _ => None
            };
            match value {
                Some(x) => Ok(Call::Put { key: key.to_string(), value: x }),
                None => Err(CLIErr::BadVal),
            }
        }
        ["del", key] => Ok(Call::Del(key.to_string())),
        ["exit"] => Ok(Call::Exit),
        _ => Err(CLIErr::BadCall),
    }
}
