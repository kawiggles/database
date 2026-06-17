pub mod store;
pub mod cli;
pub mod logs;
pub mod tcp;

use log::{info};
use std::net::TcpListener;

use crate::store::{DEFAULT_FILE, Store};
use crate::logs::{init_logs, DbError};
use crate::tcp::handle_client;

pub const VERSION: u32 = encode_version((0,0,1));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub fn run() -> Result<(), DbError> {
    init_logs();
    let mut db = Store::start(DEFAULT_FILE)?;
    info!("Database initialized");

    let listener = TcpListener::bind("127.0.0.1:55555")?;
    info!("Server listening on port");

    let mut run = true;
    while run {
        for stream in listener.incoming() {
            run = handle_client(stream?, &mut db)?;
        }
    }
    info!("Server stopping!");
    println!("Exiting program...");
    Ok(())
}

