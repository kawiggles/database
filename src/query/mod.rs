pub mod lexer;
pub mod ast;

use crate::{
    store::{Store, value::Value},
    errors::{UserResult, DbResult, StoreErr},
    query::{
        lexer::lexerize,
        ast::make_ast,
    },
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
        let tokens = lexerize(input)?;
        let ast = make_ast(tokens);
        todo!();
    }

    pub fn execute(self, db: &RwLock<Store>) -> DbResult<Value> {
        info!(" - Executing call");
        match self {
            Query::Get(key) => db.read().unwrap().get(&key),
            Query::Put { key, value } => db.write().unwrap().put(&key, value),
            Query::Del(key) => db.write().unwrap().del(&key),
        }
    }
}
