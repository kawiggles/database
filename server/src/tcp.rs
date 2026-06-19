use std::net::TcpStream;
use std::io::{BufRead, BufReader, Write};
use log::{info, warn};

use crate::cli::Call;
use crate::logs::DbError;
use crate::store::{Store};

// TODO: refactor to handle different classes of errors better
pub fn handle_client(mut stream: TcpStream, db: &mut Store) -> bool {
    loop {
        let incoming = match stream_as_string(&stream) {
            Ok(s) => s,
            Err(_) => {
                info!("Client disconnected");
                return true;
            },
        };

        let mut respond = |msg: &str| -> bool {
            stream.write_all(msg.as_bytes()).is_ok() && stream.flush().is_ok()
        };

        info!("Recieved API call: {}", incoming);
        let call_result: Result<Call, DbError> = Call::parse(&incoming);

        match call_result {
            Ok(call) => {
                let response = match call.execute(db) {
                    Ok(x) => x.print(),
                    Err(x) => format!("Error encountered: {}", x),
                };
                info!("Sending response: {}", response);
                if !respond(&response) { return true; }

                if call == Call::Exit {
                    info!("Exit call parsed");
                    return false;
                }
            },
            Err(err) => {
                warn!("Invalid call made: {}", err);
                let response = format!("Error: {}\n", err);
                if !respond(&response) { return true; }
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
