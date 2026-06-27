use crate::tcp::request::{Request, StartupMessage};
use crate::tcp::response::Response;
use crate::store::Store;
use crate::logs::Result;

use std::sync::{Arc, RwLock};

// This might have to be a vector of result responses
pub fn translate_startup(message: StartupMessage, db: &mut Arc<RwLock<Store>>) -> Result<Response> {
}

pub fn translate(request: Request, db: &mut Arc<RwLock<Store>>) -> Result<Response> {
}
