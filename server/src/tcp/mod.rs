pub mod request;
pub mod response;
pub mod translation;

use tokio::{
    net::TcpListener,
    io::{AsyncRead, AsyncWriteExt, ErrorKind},
};
use std::{
    pin::pin,
    sync::{Arc, RwLock, mpsc},
    thread,
    time::Duration,
};
use log::{info, warn};

use crate::logs::{init_logs};
use crate::store::Store;
use crate::tcp::{
    request::{decode_startup, decode_request},
    response::{encode, Response},
    translation::{translate, translate_startup},
};
use crate::cli::run_cli;

pub const DEFAULT_FILE: &str = "kawika.db";
pub const DEFAULT_ORDER: usize = 150;
pub const DEFAULT_PORT: &str = "127.0.0.1:5432";

pub struct Server {
    pub store: Arc<RwLock<Store>>,
    listener: TcpListener,
}

impl Drop for Server {
    fn drop(&mut self) {
        let mut store = self.store.write().expect("Failed to write Store cache when closing");
        store.exit().unwrap()
    }
}

impl Server {
    pub async fn start() -> Self {
        init_logs();
        info!("Starting server...");

        // TODO: add way to configure file and order selection
        let db = Arc::new(RwLock::new(Store::start(DEFAULT_FILE, DEFAULT_ORDER)
                .expect("Failed to start database")));
        info!(" - Database initialized");

        // TODO: add way to select where the server is being hosted
        let listener = TcpListener::bind(DEFAULT_PORT).await
            .expect("Failed to start TcpListener on selected port");
        info!(" - Server listening on port {}", DEFAULT_PORT);

        Server {
            store: db,
            listener: listener,
        }
    }

    pub async fn run(self) {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let tx_ctrlc = shutdown_tx.clone();
        ctrlc::set_handler(move || {
            warn!("ctrl_c registered...");
            let _ = tx_ctrlc.send(());
        }).expect("Failed to start ctrlc handler");

        let tx_cli = shutdown_tx.clone();
        let cli_db = Arc::clone(&self.store);
        thread::spawn(move || {
            info!("Starting CLI");
            loop {
                if run_cli(cli_db.as_ref()) == false {
                    let _ = tx_cli.send(());
                    break;
                }
            }
        });

        loop {
            if shutdown_rx.try_recv().is_ok() {
                warn!("Shutting down server...");
                break;
            }
            
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    let db = Arc::clone(&self.store);
                    tokio::spawn(async move {
                        info!("Connection initialized, starting thread");
                        handle_connection(pin!(stream), db).await;
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
async fn handle_connection<T: AsyncRead + AsyncWriteExt>(mut stream: T, mut db: Arc<RwLock<Store>>) 
where T: AsyncRead + AsyncWriteExt + Unpin {
    let startup = decode_startup(&mut stream).await.unwrap(); // Handle this error 
    let responses = translate_startup(startup, &mut db);
    for response in responses {
        // should probably make this a match
        let _ = stream.write_all(encode(response).unwrap().as_slice()).await.unwrap();
    }
    
    loop {
        let request = decode_request(&mut stream).await.unwrap(); // This one
        let responses = translate(request, &mut db).unwrap_or_else(|e| {
            vec![e.gen_error_response()] 
        });

        if let Response::Terminate = responses[0] {
            warn!(" - Terminating session...\n");
            break;
        }

        for response in responses {
            stream.write_all(encode(response).unwrap().as_slice()).await.unwrap(); // and this one
        }
    }
}
