use std::net::TcpStream;
use std::io::{BufRead, BufReader, Write};
use log::{info, warn};

use crate::cli::Call;
use crate::logs::DbError;
use crate::store::{Store};

// TODO: refactor to handle different classes of errors better
// TODO: place loop inside of this function instead of outside
pub fn handle_client(mut stream: TcpStream, db: &mut Store) -> bool {
    let incoming = stream_as_string(&stream).unwrap();
    info!("Recieved API call: {}", incoming);
    let call_result: Result<Call, DbError> = Call::parse(&incoming);
    info!("Parsing call");

    match call_result {
        Ok(call) => {
            let response = match call.execute(db) {
                Ok(x) => x.print(),
                Err(x) => format!("Error encountered: {}", x).to_string(),
            };
            info!("Sending response: {}", response);
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();

            if call == Call::Exit {
                info!("Exit call parsed");
                return false;
            }
        },
        Err(err) => {
            warn!("Invalid call made: {}", err);
            eprintln!("Error: {}", err);
        },
    }
    true
}

fn stream_as_string(stream: &TcpStream) -> Result<String, DbError> {
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}
