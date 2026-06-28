use crate::logs::{DbError, Result};
use crate::cli::Call;

use std::io::Read;

#[derive(Debug)]
pub struct StartupMessage(usize); // where value is version

#[derive(Debug)]
pub enum Request {
    Query(Call),
}

pub fn decode_startup<T: Read>(stream: &mut T) -> Result<StartupMessage> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    let len = usize::try_from(i32::from_be_bytes(buf))?;
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    let version = usize::try_from(i32::from_be_bytes(buf))?;

    if len == 8 {
        Ok(StartupMessage(version))
    } else {
        Err(DbError::MessageLenError)
    }
}

pub fn decode_request<T: Read>(stream: &mut T) -> Result<Request> {
    let mut buf: [u8; 1] = [0u8; 1];
    stream.read_exact(&mut buf)?;
    let message_type = buf[0] as char;

    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    let len = buf[0] as i32;
    let mut contents = vec![0; len as usize];
    stream.read_exact(&mut contents)?;

    match message_type {
        // TODO: replace with updated querying system
        'Q' => Ok(Request::Query(Call::parse(&String::from_utf8(contents)?)?)),
        _ => Err(DbError::BadMessageType)
    }
}
