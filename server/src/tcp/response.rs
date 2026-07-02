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
        pid: i32,
        key: Vec<u8>,
    },
    ParameterStatus {
        name: String,
        val: String,
    },
    // These are Normal mode responses
    ReadyForQuery(ServerState), // Technically also startup response
    RowDescription {
        field_count: i16,
        fields: Vec<RowField>,
    },
    DataRow {
        column_count: i16,
        cells: Vec<Option<Vec<u8>>>,
    },
    CommandComplete(String), // Where string is command tag
}

#[derive(Debug)]
pub enum ServerState {
    Idle,
    Transaction,
    Error,
}

#[derive(Debug)]
pub struct RowField {
    name: String,
    table_id: i32,
    _attr_num: i16,
    data_type_id: i32,
    data_type_size: i16,
    type_mod: i32,
    format: i16, // This is the different value type: 0 for text and 
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
        Response::RowDescription { field_count, fields } => enc_row_description(field_count, fields),
        Response::DataRow { column_count, cells } => enc_data_row(column_count, cells),
        Response::CommandComplete(tag) => enc_command_complete(tag),
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

fn enc_backend_key_data(pid: i32, key: Vec<u8>) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(key.len() + 8)?; // Should always work
    buf.push(b'K');
    buf.extend(len.to_be_bytes());
    buf.extend(pid.to_be_bytes()); // FUCK YEAH RUST!
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

fn enc_row_description(count: i16, fields: Vec<RowField>) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'T');
    buf.extend(0i32.to_be_bytes());
    buf.extend(count.to_be_bytes());

    for field in fields {
        buf.extend(field.name.into_bytes());
        buf.extend(field.table_id.to_be_bytes());
        buf.extend(field.data_type_id.to_be_bytes());
        buf.extend(field.data_type_size.to_be_bytes());
        buf.extend(field.type_mod.to_be_bytes());
        buf.extend(field.format.to_be_bytes());
    }

    let len = (buf.len() - 1) as i32;
    buf[1..5].copy_from_slice(&len.to_be_bytes());
    Ok(buf)
}

fn enc_data_row(count: i16, cells: Vec<Option<Vec<u8>>>) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'D');
    buf.extend(0i32.to_be_bytes());
    buf.extend(count.to_be_bytes());

    for cell in cells {
        match cell {
            Some(data) => {
                let cell_len = i32::try_from(data.len())?;
                buf.extend(cell_len.to_be_bytes());
                buf.extend(data);
            },
            None => buf.extend((-1i32).to_be_bytes()), // handles null data in tables
        }
    }

    let len = (buf.len() - 1) as i32;
    buf[1..5].copy_from_slice(&len.to_be_bytes());
    Ok(buf)
}

fn enc_command_complete(tag: String) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(tag.len() + 4)?;
    buf.push(b'C');
    buf.extend(len.to_be_bytes());
    buf.extend(tag.into_bytes());
    Ok(buf)
}
