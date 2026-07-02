use crate::VERSION;
use crate::tcp::request::{Request, StartupMessage};
use crate::tcp::response::{Response, ServerState};
use crate::store::Store;
use crate::store::value::Value;
use crate::logs::Result;

use std::sync::{Arc, RwLock};

pub fn translate_startup(_message: StartupMessage, _db: &mut Arc<RwLock<Store>>) -> Vec<Response> {
    // TODO: handle potential error instead of just saying "yeah we good"
    vec![
        Response::AuthenticationOk,
        Response::ParameterStatus { name: "server_version".into(), val: VERSION.to_string() },
        Response::ParameterStatus { name: "client_encoding".into(), val: "UTF8".into() },
        Response::ParameterStatus { name: "DataStyle".into(), val: "ISO, MDY".into() },
        Response::ParameterStatus { name: "integer_datetimes".into(), val: "on".into() },
        // TODO: get actual process id and generate a key
        Response::BackendKeyData { pid: 0, key: vec![0u8] },
        Response::ReadyForQuery(ServerState::Idle),
    ]
}

pub fn translate(request: Request, db: &mut Arc<RwLock<Store>>) -> Result<Vec<Response>> {
    match request {
        Request::Query(query) => {
            let val: Value = query.execute(db.as_ref())?;
            Ok(vec![
                Response::DataRow { 
                    column_count: 1, // This is one for now, but needs to be alterable
                    cells: vec![Some(val.to_bytes())],
                },
                Response::CommandComplete("".to_string()), // Need to pass original request here
                Response::ReadyForQuery(ServerState::Idle)
            ])
        },
    }
}
