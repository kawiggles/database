pub mod lexer;
pub mod ast;

use crate::{
    store::{Store, value::Value},
    errors::{UserResult, UserErr, DbResult, StoreErr},
    query::lexer::lexerize,
};

use log::{info};
use std::sync::RwLock;

#[derive(Debug, PartialEq)]
pub enum Query {
    Get(String),
    Put {
        key: String,
        value: Value,
    },
    Del(String),
}

impl Query {
    pub fn parse(input: &[u8]) -> UserResult<Self> {
        if !(input.is_ascii()) {
            return Err(UserErr::BadQuery);
        }

        let tokens = lexerize(input)?;
        todo!();
    }

    pub fn execute(self, db: &RwLock<Store>) -> DbResult<Value> {
        info!(" - Executing call");
        match self {
            Query::Get(key) => db.read().map_err(|_| StoreErr::PoisonError)?.get(&key),
            Query::Put { key, value } => db.write()
                .map_err(|_| StoreErr::PoisonError)?
                .put(&key, value),
            Query::Del(key) => db.write().map_err(|_| StoreErr::PoisonError)?.del(&key),
        }
    }
}
