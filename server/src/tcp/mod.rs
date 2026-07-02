pub mod request;
pub mod response;
pub mod translation;

use std::net::TcpListener;
use std::sync::{Arc, RwLock};
use std::thread;
use std::io::{Read, Write};
use log::{info};

use crate::logs::{init_logs, Result};
use crate::store::Store;
use crate::tcp::request::{decode_startup, decode_request};
use crate::tcp::response::encode;
use crate::tcp::translation::{translate, translate_startup};

pub const DEFAULT_FILE: &str = "kawika.db";
pub const DEFAULT_PORT: &str = "127.0.0.1:55555";

pub struct Server {
    store: Arc<RwLock<Store>>,
    listener: TcpListener,
}

impl Drop for Server {
    fn drop(&mut self) {
        info!("Server stopping!");
        let mut store = self.store.write().unwrap();
        store.exit().unwrap()
    }
}

// TODO: switch all the async out for tokio
impl Server {
    pub fn start() -> Result<Self> {
        init_logs();

        // TODO: add way to configure file selection
        let db = Arc::new(RwLock::new(Store::start(DEFAULT_FILE)?));
        info!("Database initialized");

        // TODO: add way to select where the server is being hosted
        let listener = TcpListener::bind(DEFAULT_PORT)?;
        info!("Server listening on port {}", DEFAULT_PORT);
        Ok(Server {
            store: db,
            listener: listener,
        })
    }

    pub fn run(self) {
        // TODO: add thread for local cli interfacing

        for stream in self.listener.incoming() {
            let db = Arc::clone(&self.store);
            thread::spawn(move || {
                handle_connection(stream.unwrap(), db);
            });
        }
    }
}

// TODO: refactor to handle different classes of errors better
// The idea is that this is where error messages decide how they get dispatched,
// depending on what kind of error they are, and who fucked up
fn handle_connection<T: Read + Write>(mut stream: T, mut db: Arc<RwLock<Store>>) {
    let startup = decode_startup(&mut stream).unwrap(); // Handle this error 
    let responses = translate_startup(startup, &mut db);
    for response in responses {
        stream.write_all(encode(response).unwrap().as_slice()).unwrap(); // And this one
    }
    
    loop {
        // Same schtick with error handling here
        let request = decode_request(&mut stream).unwrap();
        let responses = translate(request, &mut db).unwrap();
        for response in responses {
            // Inner unwrap will determine whether an errorresponse is written
            stream.write_all(encode(response).unwrap().as_slice()).unwrap();
        }
    }
}
