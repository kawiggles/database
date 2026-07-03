use crate::logs::{TcpErr, DbErr};
use crate::query::Query;
use log::{info};

use std::io::{Read, Write};

#[derive(Debug)]
pub struct StartupMessage(usize); // where value is version

#[derive(Debug)]
pub enum Request {
    Query(Query),
}

pub fn decode_startup<T: Read + Write>(stream: &mut T) -> Result<StartupMessage, TcpErr> {
    let _len = read_i32(stream)?;
    let code = read_i32(stream)?;

    match code {
        // SSL negotiation, WAYYYY down the line
        80877103 => {
            info!("Client connection received");
            stream.write_all(&[b'N'])?;
            decode_startup(stream)
        },
        196608 => {
            info!("Sending StartupMessage to client");
            Ok(StartupMessage(code as usize))
        },
        _ => Err(TcpErr::StartupMessageError)
    }
}

pub fn decode_request<T: Read>(stream: &mut T) -> Result<Request, DbErr> {
    let message_type = read_char(stream)?;

    let len = read_i32(stream)?;
    let contents = read_contents(stream, len)?;

    match message_type {
        // TODO: replace with updated querying system
        'Q' => Ok(Request::Query(Query::parse(&contents)?)),
        _ => Err(TcpErr::BadMessageType)?
    }
}

fn _read_i16<T: Read>(stream: &mut T) -> Result<i16, TcpErr> {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf)?;
    Ok(i16::from_be_bytes(buf))
}

fn read_i32<T: Read>(stream: &mut T) -> Result<i32, TcpErr> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    Ok(i32::from_be_bytes(buf))
}

// The len being passed is the len value from the message
fn read_contents<T: Read>(stream: &mut T, len: i32) -> Result<String, TcpErr> {
    let mut buf = vec![0; (len - 4) as usize];
    stream.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

fn read_char<T: Read>(stream: &mut T) -> Result<char, TcpErr> {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf)?;
    Ok(buf[0] as char)
}
