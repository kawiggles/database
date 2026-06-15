pub mod store;
pub mod cli;
pub mod logs;

use crate::store::{DEFAULT_FILE, Store};
use crate::cli::{get_input, Call};
use crate::logs::{init_logs, DbError};

use log::{info, warn};

pub const VERSION: u32 = encode_version((0,0,1));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub fn run() {
    init_logs();
    let mut db = match Store::start(DEFAULT_FILE) {
        Ok(x) => x,
        Err(e) => panic!("Error starting database! {}", e),
    };

    info!("Database initialized");
    loop {
        info!("Parsing Call");
        let call_result: Result<Call, DbError> = Call::parse(get_input().as_str());

        match call_result {
            Ok(call) => {
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
                if call == Call::Exit {
                    info!("Exit call parsed");
                    break;
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

