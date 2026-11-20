use crate::{
    errors::{DbResult, UserErr}
    store::{
        value::Value,
        catalog::Catalog,
        relation::schema::Type,
    },
    query::ast::{BOp, UOp, Statement, Create, Drop, Select, Insert, Update, Delete, Expr},
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

struct BoundSelect {
    table: usize,
    projection: Vec<BoundProjection>,
    filter: Option<BoundExpr>
}

struct BoundInsert {
    table: usize,
    columns: Vec<usize>, // indicies of columns
    values: Vec<Vec<BoundExpr>>,
}

struct BoundUpdate {
    table: usize,
    assignments: Vec<(usize, BoundExpr)>, // (column index, value)
    filter: Option<BoundExpr>
}

struct BoundDelete {
    table: usize,
    filter: Option<BoundExpr>
}

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

struct BoundProjection {
    expr: BoundExpr,
    name: String, // alias or column name
}

impl BoundStatement {
    pub fn bind_statement(statement: Statement, catalog: &Catalog) -> DbResult<Self> {
        match statement {
            Statement::Create(stmt) => Ok(BoundStatement::Create(bind_create(stmt, catalog)?)),
            Statement::Drop(stmt) => Ok(BoundStatement::Drop(bind_drop(stmt, catalog)?)),
            Statement::Select(stmt) => Ok(BoundStatement::Select(bind_select(stmt, catalog)?)),
            Statement::Insert(stmt) => Ok(BoundStatement::Insert(bind_insert(stmt, catalog)?)),
            Statement::Update(stmt) => Ok(BoundStatement::Update(bind_update(stmt, catalog)?)),
            Statement::Delete(stmt) => Ok(BoundStatement::Delete(bind_delete(stmt, catalog)?)),
        }
    }
}

fn bind_create(statement: Create, catalog: &Catalog) -> DbResult<BoundCreate> {
    if catalog.lookup_by_name(&statement.name).is_ok() {
        return Err(UserErr::TableExists)?;
    }

    let mut primary_key = None;
    statement.columns.iter().enumerate();


    Ok(BoundCreate { name: statement.name, columns: (), primary_key })
}

fn bind_drop(statement: Drop, catalog: &Catalog) -> DbResult<BoundDrop> {
    todo!()
}

fn bind_select(statement: Select, catalog: &Catalog) -> DbResult<BoundSelect> {
    todo!()
}

fn bind_insert(statement: Insert, catalog: &Catalog) -> DbResult<BoundInsert> {
    todo!()
}

fn bind_update(statement: Update, catalog: &Catalog) -> DbResult<BoundUpdate> {
    todo!()
}

fn bind_delete(statement: Delete, catalog: &Catalog) -> DbResult<BoundDelete> {
    todo!()
}

fn bind_expr(expr: Expr, catalog: &Catalog) -> DbResult<BoundExpr> {
    todo!()
}
