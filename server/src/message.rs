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
    ErrorResponse,
    AuthenticationOk,
    BackendKeyData {
        pid: usize,
        key: [u8; 256],
    },
    ReadyForQuery { }
}

impl Response {
    pub fn encode(&self) -> Vec<u8>  {
        let mut buf: Vec<u8> = Vec::new();
        match self {
            Self::ErrorResponse => {
                // TODO: figure out error handling
                buf.push(b'E');
                buf.extend(4i32.to_be_bytes());
            },
            Self::AuthenticationOk => {
                buf.push(b'R');
                buf.extend(8i32.to_be_bytes());
                buf.extend(0i32.to_be_bytes());
            },
            Self::BackendKeyData => {
                buf.push(b'K');
                buf.extend(0i32.to_be_bytes()); // extend to middle of vector
                buf.extend(
            },
            Self::ReadyForQuery => {
            },
        }
        buf
    }
}

enum ServerStatus {
    I,
    T,
    E,
}
