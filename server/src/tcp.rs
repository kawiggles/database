use std::net::TcpStream;
use std::io::{Read, BufReader, Write};
use log::{info, warn};

use crate::cli::Call;
use crate::logs::DbError;
use crate::store::{Store};

pub fn handle_client(mut stream: TcpStream, db: &mut Store) -> Result<bool, DbError> {
    info!("Parsing Call");
    let call_result: Result<Call, DbError> = Call::parse(&stream_as_string(&stream)?);

    match call_result {
        Ok(call) => {
            let response = call.execute(db)?.print();
            info!("Sending response: {}", response);
            stream.write_all(response.as_bytes())?;
            stream.flush()?;

            if call == Call::Exit {
                info!("Exit call parsed");
                return Ok(false);
            }
        },
        Err(err) => {
            warn!("Invalid call made: {}", err);
            eprintln!("Error: {}", err);
        },
    }
    Ok(true)
}

fn stream_as_string(stream: &TcpStream) -> Result<String, DbError> {
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_to_string(&mut line)?;
    Ok(line)
}
