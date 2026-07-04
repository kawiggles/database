use std::io;
use std::sync::RwLock;
use log::{info, warn};

use crate::store::Store;
use crate::store::value::Value;

pub fn run_cli(db: &RwLock<Store>) -> bool {
    info!("Starting local server cli");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap(); // Maybe handle this? but shouldn't happen
    info!("CLI input gotten as {}", input);

    let cli = parse_cli(&input);
    if let Cli::Stop = cli {
        return false;
    }

    exec_cli(cli, db);

    true
}

enum Cli {
    PrintTree,
    PrintNodes,
    Get(String),
    Set {
        key: String,
        value: Value,
    },
    Del(String),
    Stop,
    Help,
    Unknown
}

fn parse_cli(input: &str) -> Cli {
    let args: Vec<&str> = input.split_whitespace().collect();

    match args[0] {
        "print" => {
            match args[1] {
                "tree" => {
                    info!("Cli command parsed as 'print tree'...");
                    Cli::PrintTree
                },
                "nodes" => {
                    info!("Cli command parsed as 'print nodes'...");
                    Cli::PrintNodes
                },
                _ => {
                    warn!("Cli command unrecognized");
                    Cli::Unknown
                }
            }
        },
        "get" => {
            info!("Cli command parsed as 'get {}'", args[1]);
            Cli::Get(args[1].to_string())
        },
        "set" => {
            info!("Cli command parsed as 'set {} {}'", args[1], args[2]);
            Cli::Set {
                key: args[1].to_string(),
                value: Value::Text(args[2].to_string()),
            }
        },
        "del" => {
            info!("Cli command parsed as 'del {}'", args[1]);
            Cli::Del(args[1].to_string())
        },
        "stop" => {
            warn!("Command to stop server received...");
            Cli::Stop
        },
        "help" => {
            info!("Cli command parsed as 'help'");
            Cli::Help
        },
        "" => {
            warn!("No Input detected, probably an error with read_line");
            Cli::Unknown
        },
        _ => {
            warn!("Command unrecognized");
            Cli::Unknown
        }
    }
}

fn exec_cli(cli: Cli, db: &RwLock<Store>) {
    match cli {
        Cli::PrintTree => {
        },
        Cli::PrintNodes => {
        },
        Cli::Get(key) => {
            match db.read().unwrap().get(&key) {
                Ok(val) => println!("Value: {}", val.print()),
                Err(err) => println!("Error when getting value: {err}"),
            }
        },
        Cli::Set { key, value } => {
            match db.write().unwrap().put(&key, value) {
                Ok(val) => println!("Value: {}", val.print()),
                Err(err) => println!("Error when setting value: {err}"),
            }
        },
        Cli::Del(key) => {
            match db.write().unwrap().del(&key) {
                Ok(val) => println!("Value: {}", val.print()),
                Err(err) => println!("Error when setting value: {err}"),
            }
        }
        Cli::Stop => return,
        Cli::Help => {
        },
        Cli::Unknown => return,
    }
}
