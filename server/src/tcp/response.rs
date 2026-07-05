use crate::logs::TcpErr;
use log::{info};

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
        key: i32,
    },
    ParameterStatus {
        name: String,
        val: String,
    },
    // These are Normal mode responses
    ReadyForQuery(ServerState), // Technically also startup response
    RowDescription(Vec<RowField>),
    DataRow(Vec<Option<Vec<u8>>>),
    CommandComplete(String), // Where string is command tag
    Terminate,
}

#[derive(Debug)]
pub enum ServerState {
    Idle,
    Transaction,
    Error,
}

#[derive(Debug)]
pub struct RowField {
    pub name: String,
    pub table_id: i32,
    pub _attr_num: i16,
    pub data_type_id: i32,
    pub data_type_size: i16,
    pub type_mod: i32,
    pub format: FieldFormat,
}

#[derive(Debug)]
pub enum FieldFormat {
    Text = 0,
    Binary = 1,
}

impl FieldFormat {
    fn to_be_bytes(self) -> [u8; 2] {
        match self {
            Self::Text => return 0i16.to_be_bytes(),
            Self::Binary => return 1i16.to_be_bytes(),
        }
    }
}

/* TODO: Server error messaging

#[derive(Debug)]
enum ErrorFieldType {
    Severity(String),
}

*/

pub fn encode(response: Response) -> Result<Vec<u8>, TcpErr> {
    match response {
        Response::ErrorResponse => Ok(enc_error_response()),
        Response::AuthenticationOk => Ok(enc_authentication_ok()),
        Response::BackendKeyData { pid, key } => Ok(enc_backend_key_data(pid, key)),
        Response::ReadyForQuery(state) => Ok(enc_ready_for_query(state)),
        Response::ParameterStatus { name, val } => enc_parameter_status(name, val),
        Response::RowDescription(fields) => enc_row_description(fields),
        Response::DataRow(cells) => enc_data_row(cells),
        Response::CommandComplete(tag) => enc_command_complete(tag),
        Response::Terminate => Ok(vec![]),
    }
}

fn enc_error_response() -> Vec<u8> {
    info!(" - Encoding error response");
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'E');
    buf.extend(4i32.to_be_bytes());
    buf
}

fn enc_authentication_ok() -> Vec<u8> {
    info!(" - Encoding authentication ok");
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'R');
    buf.extend(8i32.to_be_bytes());
    buf.extend(0i32.to_be_bytes());
    buf
}

fn enc_backend_key_data(pid: i32, key: i32) -> Vec<u8> {
    info!(" - Encoding backend key data");
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'K');
    buf.extend(12i32.to_be_bytes());
    buf.extend(pid.to_be_bytes());
    buf.extend(key.to_be_bytes());
    buf
}

fn enc_ready_for_query(state: ServerState) -> Vec<u8> {
    info!(" - Encoding ready for query");
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

fn enc_parameter_status(name: String, val: String) -> Result<Vec<u8>, TcpErr> {
    info!(" - Encoding parameter status: {}, {}", name, val);
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(name.as_bytes());
    body.push(0);
    body.extend_from_slice(val.as_bytes());
    body.push(0);

    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'S');
    buf.extend_from_slice(&((body.len() + 4) as i32).to_be_bytes());
    buf.extend_from_slice(&body);

    Ok(buf)
}

fn enc_row_description(fields: Vec<RowField>) -> Result<Vec<u8>, TcpErr> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'T');
    buf.extend(0i32.to_be_bytes());
    buf.extend((fields.len() as i16).to_be_bytes());

    for field in fields {
        buf.extend(field.name.into_bytes());
        buf.push(0);
        buf.extend(field.table_id.to_be_bytes());
        buf.extend(field._attr_num.to_be_bytes());
        buf.extend(field.data_type_id.to_be_bytes());
        buf.extend(field.data_type_size.to_be_bytes());
        buf.extend(field.type_mod.to_be_bytes());
        buf.extend(field.format.to_be_bytes());
    }

    let len = (buf.len() - 1) as i32;
    buf[1..5].copy_from_slice(&len.to_be_bytes());
    Ok(buf)
}

fn enc_data_row(cells: Vec<Option<Vec<u8>>>) -> Result<Vec<u8>, TcpErr> {
    let mut buf: Vec<u8> = Vec::new();
    buf.push(b'D');
    buf.extend(0i32.to_be_bytes());
    buf.extend((cells.len() as i16).to_be_bytes());

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

fn enc_command_complete(tag: String) -> Result<Vec<u8>, TcpErr> {
    let mut buf: Vec<u8> = Vec::new();
    let len = i32::try_from(tag.len() + 5)?;
    buf.push(b'C');
    buf.extend(len.to_be_bytes());
    buf.extend(tag.into_bytes());
    buf.push(0);
    Ok(buf)
}
