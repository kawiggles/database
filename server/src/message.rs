use crate::store::PAGE_SIZE;
use crate::logs::{DbError, Result};

use std::io::Read;

#[derive(Debug)]
pub enum Request {
    CopyData {
        bytes: [u8; PAGE_SIZE - 9],
    },
}

impl Request {
    pub fn decode<T: Read>(stream: &mut T) -> Self {
    }
}

#[derive(Debug)]
pub enum Response {
    // These are startup responses
    ErrorResponse /* {
        TODO: Server error messaging
        field_type: ErrorFieldType,
        error: String,
    } */,
    AuthenticationOk,
    BackendKeyData {
        pid: usize,
        key: Vec<u8>,
    },
    ReadyForQuery { 
        state: ServerState
    },
}

#[derive(Debug)]
enum ServerState {
    Idle,
    Transaction,
    Error,
}

/* TODO: Server error messaging

#[derive(Debug)]
enum ErrorFieldType {
    Severity(String),
}

*/

impl Response {
    pub fn encode(&self) -> Vec<u8>  {
        match self {
            Self::ErrorResponse => enc_error_response(),
            Self::AuthenticationOk => enc_authentication_ok(),
            Self::BackendKeyData { pid, key } => enc_backend_key_data(*pid, key),
            Self::ReadyForQuery { state } => enc_ready_for_query(state),
        }
    }
}

fn enc_error_response() -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'E');
    buf.extend(4i32.to_be_bytes());
    buf
}

fn enc_authentication_ok() -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'R');
    buf.extend(8i32.to_be_bytes());
    buf.extend(0i32.to_be_bytes());
    buf
}

fn enc_backend_key_data(pid: usize, key: &Vec<u8>) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(key.len() + 8).unwrap(); // Should always work
    buf.push(b'K');
    buf.extend(len.to_be_bytes());
    buf.extend(i32::try_from(pid).unwrap().to_be_bytes()); // FUCK YEAH RUST!
    buf.extend(key.iter());
    buf
}

fn enc_ready_for_query(state: &ServerState) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'Z');
    buf.extend(5i32.to_be_bytes());
    match state {
        ServerState::Idle => buf.push(b'I'),
        ServerState::Transaction => buf.push(b'T'),
        ServerState::Error => buf.push(b'E'),
    }
    buf
}
