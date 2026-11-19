use crate::{
    errors::DbResult,
    store::{
        value::Value,
        catalog::Catalog,
        relation::schema::Type,
    },
    query::ast::{BOp, UOp, Statement},
};

pub enum BoundStatement {
    Create(BoundCreate),
    Drop(BoundDrop),
    Select(BoundSelect),
    Insert(BoundInsert),
    Update(BoundUpdate),
    Delete(BoundDelete),
}

struct BoundCreate {
    name: String,
    columns: Vec<BoundDefinition>,
    primary_key: Option<Vec<usize>>,
}

struct BoundDrop(usize);

struct BoundSelect;

struct BoundInsert;

struct BoundUpdate;

struct BoundDelete;

struct BoundExpr { kind: BoundExprKind, ty: Type }

enum BoundExprKind {
    Column { table: usize, index: usize },
    Literal(Value),
    Unary { op: UOp, expr: Box<BoundExpr> },
    Binary { left: Box::<BoundExpr>, op: BOp, right: Box<BoundExpr> },
    IsNull { expr: Box<BoundExpr>, negated: bool }
}

struct BoundDefinition {
    name: String,
    ty: Type,
    nullable: bool,
}

impl BoundStatement {
    pub fn bind_statement(statement: Statement, catalog: &Catalog) -> DbResult<Self> {
        todo!()
    }
}
