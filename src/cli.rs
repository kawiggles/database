use std::io::{Write, stdout, stdin};
use std::sync::RwLock;
use log::{info, warn};

use crate::{
    store::{ Store, value::Value },
    query::Query,
};

pub fn run_cli(db: &RwLock<Store>) -> bool {
    print!("> ");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap(); // Maybe handle this? but shouldn't happen
    info!("CLI input received");

    let cli = parse_cli(&input);
    if let Cli::Stop = cli {
        return false;
    }

    exec_cli(cli, db);

    true
}

enum Cli {
    PrintTree,
    ValidateTree,
    FlushPager,
    Query(Query),
    Stop,
    Help,
    Unknown
}

fn parse_cli(input: &str) -> Cli {
    let args: Vec<&str> = input.trim().split_whitespace().collect();

    if args.len() < 1 {
        return Cli::Unknown;
    }

    match args[0] {
        "print" => {
            if args.len() < 2 {
                warn!(" - Cli command missing argument");
                println!("Cli command missing argument");
                return Cli::Unknown;
            }

            match args[1] {
                "tree" => {
                    info!(" - Cli command parsed as 'print tree'...");
                    Cli::PrintTree
                },
                _ => {
                    warn!(" - Cli command not recognized");
                    println!("Cli command not recognized");
                    Cli::Unknown
                }
            }
        },
        "validate" => {
            info!(" - Cli comand parsed as 'validate'...");
            Cli::ValidateTree
        },
        "flush" => {
            info!(" - Cli command parsed as 'flush'...");
            Cli::FlushPager
        },
        "stop" => {
            warn!(" - Command to stop server received...");
            Cli::Stop
        },
        "help" => {
            info!(" - Cli command parsed as 'help'");
            Cli::Help
        },
        _ => {
            warn!(" - Command unrecognized");
            println!("Command not recognized");
            Cli::Unknown
        }
    }
}

fn exec_cli(cli: Cli, db: &RwLock<Store>) {
    info!(" - Executing command\n");
    match cli {
        Cli::PrintTree => db.read().unwrap().print_tree(),
        Cli::ValidateTree => {
            match db.read().unwrap().validate() {
                Some(x) => println!("Error encountered validating tree: {}", x),
                None => println!("Tree is valid!"),
            }
        },
        Cli::FlushPager => db.write().unwrap().pager.flush().unwrap(),
        Cli::Stop => return,
        Cli::Help => {
            println!("Commands:");
            println!(" - get <key>          - gets value by key in db");
            println!(" - set <key> <val>    - sets key to value, str by default");
            println!(" - del <key>          - removes key/value pair from db");
            println!(" - print tree         - prints text layout of db b+ tree");
            println!(" - validate           - validates b+ tree layout");
            println!(" - flush              - flush db write cache");
            println!(" - stop               - cleanly shuts down the server");
        },
        Cli::Query(query) => todo!(),
        Cli::Unknown => return,
    }
}
