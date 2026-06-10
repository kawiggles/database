pub mod store;
pub mod cli;
pub mod logs;

use crate::store::Store;
use crate::cli::{get_input, Call};
use crate::logs::{init_logs, DbError};

use log::{info, warn};

pub fn run() {
    init_logs();
    let mut db = Store::start();
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
                if call == Call::Help {
                    info!("Help call parsed");
                    println!("<call> key value value_type");
                    println!("Valid calls: get, put, del");
                    println!("Valid value types: text, int, float, blob");
                    println!(" - Format blobs with as [1,2,3]");
                    println!(" - put defaults to value_type text");
                    continue;
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

