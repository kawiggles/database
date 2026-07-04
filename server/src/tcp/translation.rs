use crate::VERSION;
use crate::tcp::request::{Request, StartupMessage};
use crate::tcp::response::{Response, ServerState, RowField, FieldFormat};
use crate::store::Store;
use crate::store::value::Value;
use crate::logs::{DbErr};

use std::sync::{Arc, RwLock};
use log::{info};

pub fn translate_startup(_message: StartupMessage, _db: &mut Arc<RwLock<Store>>) -> Vec<Response> {
    info!("Generating startup message response");
    // TODO: handle potential error instead of just saying "yeah we good"
    vec![
        Response::AuthenticationOk,
        Response::ParameterStatus { name: "server_version".into(), val: VERSION.to_string() },
        Response::ParameterStatus { name: "client_encoding".into(), val: "UTF8".into() },
        Response::ParameterStatus { name: "DateStyle".into(), val: "ISO, MDY".into() },
        Response::ParameterStatus { name: "integer_datetimes".into(), val: "on".into() },
        // TODO: get actual process id and generate a key
        Response::BackendKeyData { pid: 0, key: 0 },
        Response::ReadyForQuery(ServerState::Idle),
    ]
}

// TODO: pass CommandComplete tag from parse to this function
pub fn translate(request: Request, db: &mut Arc<RwLock<Store>>) -> Result<Vec<Response>, DbErr> {
    match request {
        Request::Query(query) => {
            // Need to come up with different classes of errors
            let val: Value = query.execute(db.as_ref())?;
            info!("Return value is {}", val.print());
            Ok(vec![
                Response::RowDescription(
                    vec![RowField{
                        name: "value".into(),
                        table_id: 0,
                        _attr_num: 0,
                        data_type_id: 17,
                        data_type_size: -1,
                        type_mod: 0,
                        format: FieldFormat::Text,
                    }]),
                Response::DataRow(vec![Some(val.to_bytes())]),
                Response::CommandComplete("".to_string()), // Need to pass original request here
                Response::ReadyForQuery(ServerState::Idle)
            ])
        },
        Request::Termination => Ok(vec![Response::Terminate]),
    }
}
