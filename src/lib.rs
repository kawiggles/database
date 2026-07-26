pub mod store;
pub mod tcp;
pub mod query;
pub mod errors;
pub mod cli;

use log::{info};

use crate::tcp::Server;

pub const VERSION: u32 = encode_version((0,1,0));

pub const fn encode_version(version: (u32, u32, u32)) -> u32 {
    (version.0 << 24) | (version.1 << 16) | (version.2 << 8)
}

pub async fn run() {
    let server = Server::start().await;

    info!("Server setup complete, running server");
    server.run().await;

    info!("Program exiting...");
    println!("Program exiting...");
}

