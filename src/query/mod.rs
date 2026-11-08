pub mod lexer;
pub mod ast;
pub mod executor;
pub mod binder;

use crate::{
    store::{ Store, relation::Row },
    errors::{DbResult},
};

use self::{
    ast::{Expr},
};

#[derive(Debug, PartialEq)]
pub enum Operator {
    SeqScan(SeqScan),
    Filter { child: Box<Operator>, pred: Expr },
    Project { child: Box<Operator>, exprs: Vec<Expr> },
    Limit { child: Box<Operator>, remaining: usize },
}

#[derive(Debug, PartialEq)]
struct SeqScan;

impl SeqScan {
    fn scan(&self) -> Vec<Row>
}

impl Operator {
    fn next(&mut self, store: &mut Store) -> DbResult<Option<Row>> {
        match self {
            Self::SeqScan(seq) => {
            },
            Self::Filter { child, pred } => {
            },
            Self::Project { child, exprs } => {
            },
            Self::Limit { child, remaining } => {
            },
        }
    }
}
