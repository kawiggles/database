use std::net::{TcpStream, TcpListener};
use std::sync::{Arc, RwLock};
use std::thread;
use std::io::{BufRead, BufReader, Write, stdin, stdout};
use log::{info, warn};

use crate::cli::Call;
use crate::logs::{init_logs, DbError};
use crate::store::{Store};

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

impl Server {
    pub fn start() -> Result<Self, DbError> {
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
        let mut local = Arc::clone(&self.store);
        thread::spawn(move || {
            let mut scanner = BufReader::new(stdin());
            let stdout = stdout();
            loop {
                print!("Database prompt: ");
                stdout.lock().flush().unwrap();

                let mut line = String::new();
                scanner.read_line(&mut line).unwrap_or_else(|err| {
                    warn!("Error reading server cli: {}", err);
                    0
                });

                let call = Call::parse(line.trim());
                match call {
                    Ok(call) => {
                        let response = match call.execute(&mut local) {
                            Ok(x) => x.print(),
                            Err(e) => format!("Error encountered: {}", e),
                        };
                        println!("{}", response);

                        if call == Call::Exit {
                            break;
                        }
                    },
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
        });

        for stream in self.listener.incoming() {
            let db = Arc::clone(&self.store);
            thread::spawn(move || {
                handle_client(stream.unwrap(), db);
            });
        }
    }
}

// TODO: refactor to handle different classes of errors better
fn handle_client(mut stream: TcpStream, mut db: Arc<RwLock<Store>>) {
    loop {
        let incoming = match stream_as_string(&stream) {
            Ok(s) => s,
            Err(_) => {
                info!("Client disconnected");
                return;
            },
        };

        let mut respond = |msg: &str| -> bool {
            stream.write_all(msg.as_bytes()).is_ok() && stream.flush().is_ok()
        };
        info!("Recieved API call: {}", incoming);
        let call_result: Result<Call, DbError> = Call::parse(&incoming);

        match call_result {
            Ok(call) => {
                let response = match call.execute(&mut db) {
                    Ok(x) => x.print(),
                    Err(x) => format!("Error encountered: {}", x),
                };
                info!("Sending response: {}", response);
                if !respond(&response) { return; }

                if call == Call::Exit {
                    info!("Exit call parsed");
                    return;
                }
            },
            Err(err) => {
                warn!("Invalid call made: {}", err);
                let response = format!("Error: {}\n", err);
                if !respond(&response) { return; }
            },
        }
    }
}

fn stream_as_string(stream: &TcpStream) -> Result<String, DbError> {
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}
