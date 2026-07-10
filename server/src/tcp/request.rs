use crate::logs::{TcpErr, DbErr};
use crate::query::Query;
use log::{info, warn};

use tokio::io::{AsyncReadExt, AsyncWriteExt, Result};

#[derive(Debug)]
pub struct StartupMessage(usize); // where value is version

#[derive(Debug)]
pub enum Request {
    Query(Query),
    Termination
}

pub async fn decode_startup<T: AsyncReadExt + Unpin + AsyncWriteExt>
    (stream: &mut T) -> Result<StartupMessage> {

    info!(" - Decoding client startup message");
    let len = read_i32(stream);
    let code = read_i32(stream);
    info!("   - Code is {}", code);

    match code {
        // SSL negotiation, WAYYYY down the line
        80877103 => {
            info!("   - Negotiating SSL with client");
            stream.write_all(&[b'N']);
            decode_startup(stream).await;
        },
        196608 => {
            info!("   - Postgresql client is using protocol version 3.0");
            // TODO: Parse user parameter
            let mut params = vec![0u8; (len - 8) as usize];
            stream.read_exact(&mut params);

            Ok(StartupMessage(code as usize))
        },
        _ => {
            warn!("   - Error decoding startup message");
            Err(TcpErr::StartupMessageError)
        },
    }
}

pub async fn decode_request<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Request, DbErr> {
    info!(" - Decoding client request");
    let message_type = read_char(stream)?;
    info!("   - Message type is {}", message_type);

    let len = read_i32(stream)?;
    let contents = read_contents(stream, len)?;
    info!("   - Message contents are {}", contents);

    match message_type {
        // TODO: replace with updated querying system
        'Q' => Ok(Request::Query(Query::parse(&contents)?)),
        'X' => Ok(Request::Termination),
        _ => {
            warn!("   - Type not recognized {}", message_type);
            Err(TcpErr::BadMessageType)?
        }
    }
}

fn _read_i16<T: AsyncReadExt + Unpin>(stream: &mut T) -> i16 {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf);
    i16::from_be_bytes(buf)
}

fn read_i32<T: AsyncReadExt + Unpin>(stream: &mut T) -> i32 {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf);
    i32::from_be_bytes(buf)
}

// TODO: validate that len is correct
// The len being passed is the len value from the message
fn read_contents<T: AsyncReadExt + Unpin>(stream: &mut T, len: i32) -> Result<String, TcpErr> {
    let mut buf = vec![0; (len - 4) as usize];
    stream.read_exact(&mut buf);
    let contents = String::from_utf8(buf)?
        .trim_matches('\0')
        .to_string();
    Ok(contents)
}

fn read_char<T: AsyncReadExt + Unpin>(stream: &mut T) -> char {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf);
    buf[0] as char
}
