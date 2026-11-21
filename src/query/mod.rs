pub mod lexer;
pub mod ast;
pub mod binder;
// pub mod planner;
pub mod executor;

use crate::errors::DbResult;
use crate::store::Store;
use crate::store::value::Value;

#[derive(Debug)]
pub struct Query;

impl Query {
    pub fn parse(bytes: &[u8]) -> DbResult<Self> {
        todo!()
    }

    pub fn execute(&self, db: &std::sync::RwLock<Store>) -> DbResult<Value> {
        todo!()
    }
}
