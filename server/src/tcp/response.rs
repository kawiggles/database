use crate::logs::{Result};

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
    ReadyForQuery(ServerState),
    ParameterStatus {
        name: String,
        val: String,
    },
}

#[derive(Debug)]
pub enum ServerState {
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

pub fn encode(response: Response) -> Result<Vec<u8>> {
    match response {
        Response::ErrorResponse => Ok(enc_error_response()),
        Response::AuthenticationOk => Ok(enc_authentication_ok()),
        Response::BackendKeyData { pid, key } => enc_backend_key_data(pid, key),
        Response::ReadyForQuery(state) => Ok(enc_ready_for_query(state)),
        Response::ParameterStatus { name, val } => enc_parameter_status(name, val),
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

fn enc_backend_key_data(pid: usize, key: Vec<u8>) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(key.len() + 8)?; // Should always work
    buf.push(b'K');
    buf.extend(len.to_be_bytes());
    buf.extend(i32::try_from(pid)?.to_be_bytes()); // FUCK YEAH RUST!
    buf.extend(key.iter());
    Ok(buf)
}

fn enc_ready_for_query(state: ServerState) -> Vec<u8> {
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

fn enc_parameter_status(name: String, val: String) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(name.len() + val.len() + 4)?;
    buf.push(b'S');
    buf.extend(len.to_be_bytes());
    buf.extend(name.into_bytes());
    buf.extend(val.into_bytes());
    Ok(buf)
}
