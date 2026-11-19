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
    pub fn parse(bytes: &[u8], db) -> DbResult<Self> {
        todo!()
    }

    pub fn execute(&self, db: &std::sync::RwLock<Store>) -> DbResult<Value> {
        let tokens = lexer::lexerize(bytes)?;
        let statement = ast::make_ast(tokens)?;
        let catalog = db.read().unwrap().catalog;
        let bound = binder::BoundStatement::bind_statement(statement, &catalog)?;
        todo!()
    }
}
