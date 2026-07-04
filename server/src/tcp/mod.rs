pub mod request;
pub mod response;
pub mod translation;

use std::net::TcpListener;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Duration;
use std::io::{Read, Write, ErrorKind};
use log::{info, warn};

use crate::logs::{init_logs};
use crate::store::Store;
use crate::tcp::request::{decode_startup, decode_request};
use crate::tcp::response::{encode, Response};
use crate::tcp::translation::{translate, translate_startup};
use crate::cli::run_cli;

pub const DEFAULT_FILE: &str = "kawika.db";
pub const DEFAULT_PORT: &str = "127.0.0.1:5432";

pub struct Server {
    pub store: Arc<RwLock<Store>>,
    listener: TcpListener,
}

impl Drop for Server {
    fn drop(&mut self) {
        let mut store = self.store.write().unwrap();
        store.exit().unwrap()
    }
}

// TODO: switch all the async out for tokio
impl Server {
    pub fn start() -> Self {
        init_logs();

        // TODO: add way to configure file selection
        let db = Arc::new(RwLock::new(Store::start(DEFAULT_FILE).unwrap_or_else(|err| {
            panic!("Error opening database file: {err}");
        })));
        info!("Database initialized");

        // TODO: add way to select where the server is being hosted
        let listener = TcpListener::bind(DEFAULT_PORT).unwrap_or_else(|err| {
            panic!("Error binding server to port: {err}");
        });
        info!("Server listening on port {}", DEFAULT_PORT);

        Server {
            store: db,
            listener: listener,
        }
    }

    pub fn run(self) {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let tx_ctrlc = shutdown_tx.clone();
        ctrlc::set_handler(move || {
            warn!("ctrl_c registered...");
            let _ = tx_ctrlc.send(());
        }).unwrap(); // TODO: Replace unwrap

        let tx_cli = shutdown_tx.clone();
        let cli_db = Arc::clone(&self.store);
        thread::spawn(move || {
            loop {
                if run_cli(cli_db.as_ref()) == false {
                    let _ = tx_cli.send(());
                    break;
                }
            }
        });

        self.listener.set_nonblocking(true).unwrap(); // TODO: Replace unwrap()
        loop {
            if shutdown_rx.try_recv().is_ok() {
                warn!("Shutting down server...");
                break;
            }
            
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let db = Arc::clone(&self.store);
                    thread::spawn(move || {
                        info!("Connection initialized, started thread");
                        handle_connection(stream, db); // TODO: Replace unwrap()
                    });
                },
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("Error encountered: {err}"),
            }
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
        let responses = translate(request, &mut db).unwrap_or_else(|_| {
            // TODO: Actually write errors to client (no unwrap, pass the Result)
            vec![Response::ErrorResponse]
        });

        if let Response::Terminate = responses[0] {
            warn!("Client terminated session");
            break;
        }

        for response in responses {
            stream.write_all(encode(response).unwrap().as_slice()).unwrap();
        }
    }
}
