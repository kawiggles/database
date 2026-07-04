pub mod store;
pub mod tcp;
pub mod query;
pub mod logs;

use log::{info, warn};
use std::sync::Arc;
use crate::tcp::Server;

pub const VERSION: u32 = encode_version((0,0,1));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub fn run() {
    let server = Server::start();
    let store = Arc::clone(&server.store);

    ctrlc::set_handler(move || {
        info!("Shutting down from ctrlc");
            store.clone()
            .as_ref()
            .write()
            .unwrap_or_else(|err| {
                panic!("Error writing to pager: {err}");
            })
            .pager.flush().unwrap_or_else(|err| {
                warn!("Error flushing pager: {err}");
            });
        std::process::exit(0);
    }).unwrap(); // Probably should handle this at some point

    info!("Server started, running server");
    server.run();

    info!("Program exiting...");
}

