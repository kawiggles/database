pub mod store;
pub mod cli;
pub mod logs;

use crate::store::Store;
use crate::cli::{get_input, Call};
use crate::logs::{init_logs, DbError};

use log::{info, warn};

pub fn run() {
    init_logs();
    let mut db = Store::build();
    info!("Database initialized");
    loop {
        info!("Parsing Call");
        let call_result: Result<Call, DbError> = Call::parse(get_input().as_str());

        match call_result {
            Ok(call) => {
                if call == Call::Exit {
                    info!("Exit call parsed");
                    break;
                }
                match call.execute(&mut db) {
                    Ok(value) => {
                        let val = value.print();
                        info!("Value retrieved: {}", val);
                        println!("{}", val);
                    },
                    Err(err) => {
                        warn!("Error in call execution: {}", err);
                        eprintln!("Error: {}", err);
                    }
                }
            }
            Err(err) => {
                warn!("Invalid call made: {}", err);
                eprintln!("Error: {}", err);
            }
        }
    }
    println!("Exiting program...");
}

