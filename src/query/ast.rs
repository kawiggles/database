use crate::{
    query::lexer::Token,
    errors::{},
};

#[derive(Debug, PartialEq)]
enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    Create(CreateStmt),
    Update(UpdateStmt),
    Copy(CopyStmt),
}


#[derive(Debug, PartialEq)]
struct SelectStmt {
    table: TableRef,
    columns: Vec<ColumnRef>,
    where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
struct InsertStmt {
    table: TableRef,
    columns: Vec<ColumnRef>,
    values: Vec<Vec<Expr>>,
}

#[derive(Debug, PartialEq)]
struct CreateStmt {
    table: TableRef,
    columns: Vec<ColumnDef>,
    as_expr: Option<SelectStmt>,
}

#[derive(Debug, PartialEq)]
struct UpdateStmt {
    table: TableRef,
    assignments: Vec<Assignment>,
    where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
struct CopyStmt {
    table: TableRef,
    target: Target,
    format: Format,
    header: bool,
}

#[derive(Debug, PartialEq)]
enum Expr {
    Literal(Literal),
    ColumnRef(ColumnRef),
    BinaryExpr { 
        left: Box<Expr>,
        operator: BOp,
        right: Box<Expr>
    },
    UnaryExpr {
        operator: UOp,
        expr: Box<Expr>,
    },
}

#[derive(Debug, PartialEq)]
enum Literal {
    Str(String),
    Int(i64),
    Null,
}

#[derive(Debug, PartialEq)]
enum TableRef {
    Table(String),
    Alias {
        alias: String,
        table: Box<TableRef>,
    },
}

#[derive(Debug, PartialEq)]
enum ColumnRef {
    AllColumns,
    Column {
        table: Option<TableRef>,
        column: String,
    },
    Alias {
        alias: String,
        column: Box<ColumnRef>,
    }
}

#[derive(Debug, PartialEq)]
enum BOp { Eq, NotEq, Lt, Gt, LtEq, GtEq, And, Or, }

#[derive(Debug, PartialEq)]
enum UOp { Not }

#[derive(Debug, PartialEq)]
struct Assignment {
    column: ColumnRef,
    val: Box<Expr>,
}

#[derive(Debug, PartialEq)]
enum Target {
    Stdin,
    Stdout,
    Path(String),
}

#[derive(Debug, PartialEq)]
enum Format {
    Csv,
}

#[derive(Debug, PartialEq)]
struct ColumnDef {
    name: String,
}

pub fn make_ast(tokens: Vec<Token>) -> Statement {
    todo!()
}
