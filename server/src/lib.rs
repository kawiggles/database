pub mod store;
pub mod tcp;
pub mod query;
pub mod logs;

use log::info;
use crate::tcp::Server;

pub const VERSION: u32 = encode_version((0,0,1));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub fn run() {
    let server = Server::start();

    info!("Server started, running server");
    server.run();

    info!("Program exiting...");
}

