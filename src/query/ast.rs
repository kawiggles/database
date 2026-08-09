use crate::{
    query::lexer::Token,
    errors::{QueryResult, QueryErr},
};

#[derive(Debug, PartialEq)]
pub enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    Create(CreateStmt),
    Update(UpdateStmt),
    Copy(CopyStmt),
}

#[derive(Debug, PartialEq)]
pub struct SelectStmt {
    table: TableRef,
    columns: Vec<ColumnRef>,
    where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct InsertStmt {
    table: TableRef,
    columns: Vec<ColumnRef>,
    values: Vec<Vec<Expr>>,
}

#[derive(Debug, PartialEq)]
pub struct CreateStmt {
    table: TableRef,
    columns: Vec<ColumnDef>,
}

#[derive(Debug, PartialEq)]
pub struct UpdateStmt {
    table: TableRef,
    assignments: Vec<Assignment>,
    where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct CopyStmt {
    table: TableRef,
    target: Target,
    format: Format,
    header: bool,
}

#[derive(Debug, PartialEq)]
pub enum Expr {
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
pub enum Literal {
    Str(String),
    Int(i64),
    Null,
}

#[derive(Debug, PartialEq)]
pub enum TableRef {
    Table(String),
    Alias {
        alias: String,
        table: Box<TableRef>,
    },
}

#[derive(Debug, PartialEq)]
pub enum ColumnRef {
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
pub enum BOp { Eq, NotEq, Lt, Gt, LtEq, GtEq, And, Or, }

#[derive(Debug, PartialEq)]
pub enum UOp { Not }

#[derive(Debug, PartialEq)]
pub struct Assignment {
    column: ColumnRef,
    val: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub enum Target {
    Stdin,
    Stdout,
    To(String),
    From(String),
}

#[derive(Debug, PartialEq)]
pub enum Format {
    Csv,
}

#[derive(Debug, PartialEq)]
struct ColumnDef {
    name: String,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser {
            tokens: tokens,
            pos: 0,
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    fn expect(&mut self, expected: &Token) -> QueryResult<()> {
        if self.peek() == expected {
            self.pos += 1;
            Ok(())
        } else {
            Err(QueryErr::UnexpectedToken {
                found: self.peek().clone(),
                expected: format!("{}", expected)
            })
        }
    }

    fn parse_statement(&mut self) -> QueryResult<Statement> {
        match self.peek() {
            Token::Select => Ok(Statement::Select(self.parse_select()?)),
            Token::Insert => Ok(Statement::Insert(self.parse_insert()?)),
            Token::Create => Ok(Statement::Create(self.parse_create()?)),
            Token::Update => Ok(Statement::Update(self.parse_update()?)),
            Token::Copy => Ok(Statement::Copy(self.parse_copy()?)),
            other => Err(QueryErr::UnexpectedToken {
                found: other.clone(),
                expected: "command token".to_string()
            }),
        }
    }

    fn parse_select(&mut self) -> QueryResult<SelectStmt> {
        self.expect(&Token::Select)?;
        let columns = self.parse_column_list()?;
        self.expect(&Token::From)?;
        let table = self.parse_table_ref()?;

        let where_clause = if self.peek() == &Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(SelectStmt {columns, table, where_clause })
    }

    fn parse_insert(&mut self) -> QueryResult<InsertStmt> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;
        let table = self.parse_table_ref()?;
        let columns = self.parse_column_list()?;
        self.expect(&Token::Values)?;
        let values = self.parse_values()?;

        Ok(InsertStmt { table, columns, values })
    }

    fn parse_create(&mut self) -> QueryResult<CreateStmt> {
        self.expect(&Token::Create)?;
        self.expect(&Token::Table)?;
        let table = self.parse_table_ref()?;
        let columns = self.parse_column_list_def()?;

        Ok(CreateStmt { table, columns })
    }

    fn parse_update(&mut self) -> QueryResult<UpdateStmt> {
        self.expect(&Token::Update)?;
        let table = self.parse_table_ref()?;
        self.expect(&Token::Set)?;
        let assignments = self.parse_assignments()?;
        
        let where_clause = if self.peek() == &Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        
        Ok(UpdateStmt { table, assignments, where_clause })
    }

    fn parse_copy(&mut self) -> QueryResult<CopyStmt> {
        self.expect(&Token::Copy)?;
        let table = self.parse_table_ref()?;
        
        let target = match self.peek() {
            Token::To => {
                self.advance();
                match self.advance() {
                    Token::Stdout => Target::Stdout,
                    Token::StringLiteral(path) => Target::To(path.to_owned()),
                    other => return Err(QueryErr::UnexpectedToken {
                        found: other.clone(),
                        expected: "STDOUT or string<path>".to_string(),
                    }),
                }
            },
            Token::From => {
                self.advance();
                match self.advance() {
                    Token::Stdin => Target::Stdin,
                    Token::StringLiteral(path) => Target::From(path.to_owned()),
                    other => return Err(QueryErr::UnexpectedToken {
                        found: other.clone(),
                        expected: "STDIN or string<path>".to_string()
                    }),
                }
            },
            other => return Err(QueryErr::UnexpectedToken { 
                found: other.clone(), 
                expected: "TO or FROM".to_string(), 
            }),
        };

        self.expect(&Token::With)?;
        let (format, header) = self.parse_with()?;

        Ok(CopyStmt { table, target, format, header })
    }

    fn parse_column_list(&mut self) -> QueryResult<Vec<ColumnRef>> {
        todo!()
    }

    fn parse_table_ref(&mut self) -> QueryResult<TableRef> {
        todo!()
    }

    fn parse_expr(&mut self) -> QueryResult<Expr> {
        todo!()
    }

    fn parse_values(&mut self) -> QueryResult<Vec<Vec<Expr>>> {
        todo!()
    }

    fn parse_column_list_def(&mut self) -> QueryResult<Vec<ColumnDef>> {
        todo!()
    }

    fn parse_assignments(&mut self) -> QueryResult<Vec<Assignment>> {
        todo!()
    }

    fn parse_with(&mut self) -> QueryResult<(Format, bool)> {
        todo!()
    }
}

pub fn make_ast(tokens: Vec<Token>) -> QueryResult<Statement> {
    let mut parser = Parser::new(&tokens);
    parser.parse_statement()
}
