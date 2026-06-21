pub mod store;
pub mod cli;
pub mod logs;
pub mod tcp;

use crate::tcp::Server;

pub const VERSION: u32 = encode_version((0,0,1));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub fn run() {
    let server = Server::start().unwrap_or_else(|err| {
        panic!("Error starting server: {}", err);
    });

    server.run();

    println!("Exiting program...");
}

