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
    Bool(bool),
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
pub struct ColumnDef {
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
                        found: other,
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
                        found: other,
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
        let mut columns = vec![self.parse_column_ref()?];

        while self.peek() == &Token::Comma {
            self.advance();
            columns.push(self.parse_column_ref()?);
        }

        Ok(columns)
    }

    fn parse_column_ref(&mut self) -> QueryResult<ColumnRef> {
        if self.peek() == &Token::Star {
            self.advance();
            return Ok(ColumnRef::AllColumns)
        }

        let first = match self.advance() {
            Token::Ident(name) => name,
            other => return Err(QueryErr::UnexpectedToken {
                found: other,
                expected: "column name or *".to_string()
            }),
        };

        let column = if self.peek() == &Token::Dot {
            self.advance();
            let col_name = match self.advance() {
                Token::Ident(i) => i,
                other => return Err(QueryErr::UnexpectedToken {
                    found: other,
                    expected: "column identifier".to_string()
                }),
            };

            ColumnRef::Column {
                table: Some(TableRef::Table(first)),
                column: col_name,
            }
        } else {
            ColumnRef::Column { 
                table: None,
                column: first
            }
        };

        if self.peek() == &Token::As {
            self.advance();
            let alias = match self.advance() {
                Token::Ident(i) => i,
                other => return Err(QueryErr::UnexpectedToken {
                    found: other,
                    expected: "an alias".to_string(),
                }),
            };

            Ok(ColumnRef::Alias { alias, column: Box::new(column) })
        } else {
            Ok(column)
        }
    }

    // TODO: parse schema dots
    fn parse_table_ref(&mut self) -> QueryResult<TableRef> {
        match self.advance() {
            Token::Ident(ident) => {
                if self.peek() == &Token::As {
                    self.advance();
                    let table = Box::new(TableRef::Table(ident));
                    let alias = match self.advance() {
                        Token::Ident(a) => a,
                        other => return Err(QueryErr::UnexpectedToken {
                            found: other,
                            expected: "ident<alias>".to_string()
                        })
                    };

                    Ok(TableRef::Alias { alias, table })
                } else {
                    Ok(TableRef::Table(ident))
                }
            },
            other => Err(QueryErr::UnexpectedToken {
                found: other,
                expected: "identity<table>".to_string()
            })
        }
    }

    fn parse_expr(&mut self) -> QueryResult<Expr> {
        let mut left = self.parse_conjunction()?;

        while self.peek() == &Token::Or {
            self.advance();
            let right = self.parse_conjunction()?;
            left = Expr::BinaryExpr {
                left: Box::new(left),
                operator: BOp::Or,
                right: Box::new(right)
            };
        }
        
        Ok(left)
    }

    fn parse_values(&mut self) -> QueryResult<Vec<Vec<Expr>>> {
        let mut rows = vec![self.parse_row()?];

        while self.peek() == &Token::Comma {
            self.advance();
            rows.push(self.parse_row()?);
        }

        Ok(rows)
    }

    fn parse_row(&mut self) -> QueryResult<Vec<Expr>> {
        self.expect(&Token::LParen)?;

        let mut row = vec![self.parse_expr()?];
        while self.peek() == &Token::Comma {
            self.advance();
            row.push(self.parse_expr()?);
        }

        self.expect(&Token::RParen)?;
        Ok(row)
    }

    fn parse_column_list_def(&mut self) -> QueryResult<Vec<ColumnDef>> {
        let mut columns = vec![self.parse_column_def()?];

        while self.peek() == &Token::Comma {
            self.advance();
            columns.push(self.parse_column_def()?);
        }

        Ok(columns)
    }

    fn parse_column_def(&mut self) -> QueryResult<ColumnDef> {
        match self.advance() {
            Token::Ident(i) => Ok(ColumnDef { name: i }),
            other => Err(QueryErr::UnexpectedToken {
                found: other,
                expected: "new column identifier".to_string(),
            }),
        }
    }

    fn parse_assignments(&mut self) -> QueryResult<Vec<Assignment>> {
        let mut assignments = vec![self.parse_assignment()?];

        while self.peek() == &Token::Comma {
            self.advance();
            assignments.push(self.parse_assignment()?);
        }

        Ok(assignments)
    }

    fn parse_assignment(&mut self) -> QueryResult<Assignment> {
        // TODO: Update target column parsing
        let column = self.parse_column_ref()?;
        self.expect(&Token::Eq)?;
        let val = self.parse_expr()?;

        Ok(Assignment {
            column: column,
            val: Box::new(val),
        })
    }

    fn parse_with(&mut self) -> QueryResult<(Format, bool)> {
        self.expect(&Token::LParen)?;
        self.expect(&Token::Format)?;
        let format = match self.advance() {
            Token::Ident(i) => {
                match i.to_ascii_lowercase().as_str() {
                    "csv" => Format::Csv,
                    _ => return Err(QueryErr::UnknownFormat(i)),
                }
            },
            other => return Err(QueryErr::UnexpectedToken {
                found: other,
                expected: "format identifier".to_string(),
            }),
        };

        let header = if self.peek() == &Token::Comma {
            self.advance();
            self.expect(&Token::Header)?;
            match self.advance() {
                Token::BoolLiteral(b) => b,
                other => return Err(QueryErr::UnexpectedToken {
                    found: other,
                    expected: "bool".to_string(),
                }),
            }
        } else {
            false
        };

        self.expect(&Token::RParen)?;
        Ok((format, header))
    }

    fn parse_conjunction(&mut self) -> QueryResult<Expr> {
        let mut left = self.parse_not()?;

        while self.peek() == &Token::And {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::BinaryExpr {
                left: Box::new(left),
                operator: BOp::And,
                right: Box::new(right)
            };
        }

        Ok(left)
    }

    fn parse_not(&mut self) -> QueryResult<Expr> {
        if self.peek() == &Token::Not {
            self.advance();
            let expr = self.parse_not()?;
            return Ok(Expr::UnaryExpr { operator: UOp::Not, expr: Box::new(expr) });
        }

        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> QueryResult<Expr> {
        let left = self.parse_primary()?;

        if let Some(op) = self.peek_binop() {
            self.advance();
            let right = self.parse_primary()?;
            return Ok(Expr::BinaryExpr {
                left: Box::new(left),
                operator: op,
                right: Box::new(right), 
            });
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> QueryResult<Expr> {
        match self.advance() {
            Token::IntLiteral(i) => Ok(Expr::Literal(Literal::Int(i))),
            Token::StringLiteral(s) => Ok(Expr::Literal(Literal::Str(s))),
            Token::BoolLiteral(b) => Ok(Expr::Literal(Literal::Bool(b))),
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            // TODO: dotted column references: if self.peek() == &Token::Dot {
            Token::Ident(n) => Ok(Expr::ColumnRef(ColumnRef::Column {
                table: None,
                column: n,
            })),
            Token::LParen => {
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            },
            other => Err(QueryErr::UnexpectedToken {
                found: other,
                expected: "literal, column reference, or '('".to_string(),
            })
        }
    }
    
    fn peek_binop(&self) -> Option<BOp> {
        match &self.tokens[self.pos] {
            Token::Eq => Some(BOp::Eq),
            Token::NotEq => Some(BOp::NotEq),
            Token::Lt => Some(BOp::Lt),
            Token::Gt => Some(BOp::Gt),
            Token::LtEq => Some(BOp::LtEq),
            Token::GtEq => Some(BOp::GtEq),
            _ => None,
        }
    }
}

pub fn make_ast(tokens: Vec<Token>) -> QueryResult<Statement> {
    // TODO: Multiline processing loop
    let mut parser = Parser::new(&tokens);
    let stmt = parser.parse_statement()?;
    parser.expect(&Token::Semicolon)?;
    Ok(stmt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::ast::{Expr};

    fn int(n: i64) -> Expr {
        Expr::Literal(Literal::Int(n))
    }

    fn str_(s: &str) -> Expr {
        Expr::Literal(Literal::Str(s.to_string()))
    }

    fn col(c: &str) -> Expr {
        Expr::ColumnRef(ColumnRef::Column { table: None, column: c.to_string() })
    }

    fn bin(left: Expr, op: BOp, right: Expr) -> Expr {
        Expr::BinaryExpr { left: Box::new(left), operator: op, right: Box::new(right) }
    }

    fn not_(e: Expr) -> Expr {
        Expr::UnaryExpr { operator: UOp::Not, expr: Box::new(e) }
    }

    #[test]
    fn basic_expr() {
        let tokens = vec![
            Token::Ident("name".to_string()),
            Token::NotEq,
            Token::StringLiteral("Falco".to_string()),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = bin(col("name"), BOp::NotEq, str_("Falco"));

        assert_eq!(parser.parse_expr().unwrap(), expected);
    }

    #[test]
    fn and_vs_or_binding() {
        let tokens = vec![
            Token::Ident("age".to_string()), Token::Eq, Token::IntLiteral(1),
            Token::Or,
            Token::Ident("age".to_string()), Token::Eq, Token::IntLiteral(2),
            Token::And,
            Token::Ident("age".to_string()), Token::Eq, Token::IntLiteral(3),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = bin(
            bin(col("age"), BOp::Eq, int(1)),
            BOp::Or,
            bin(bin(col("age"), BOp::Eq, int(2)), BOp::And, bin(col("age"), BOp::Eq, int(3))),
        );

        assert_eq!(parser.parse_expr().unwrap(), expected);
    }

    #[test]
    fn not_vs_and_binding() {
        let tokens = vec![
            Token::Not,
            Token::Ident("cost".to_string()), Token::Eq, Token::IntLiteral(5),
            Token::And,
            Token::Ident("cost".to_string()), Token::Gt, Token::IntLiteral(2),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = bin(
            not_(bin(col("cost"), BOp::Eq, int(5))),
            BOp::And,
            bin(col("cost"), BOp::Gt, int(2)),
        );

        assert_eq!(parser.parse_expr().unwrap(), expected);
    }

    #[test]
    fn basic_select() {
        let tokens = vec![
            Token::Select, Token::Star,
            Token::From, Token::Ident("table".into()),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = SelectStmt {
            table: TableRef::Table("table".to_string()),
            columns: vec![ColumnRef::AllColumns],
            where_clause: None,
        };

        assert_eq!(parser.parse_select().unwrap(), expected);
    }

    #[test]
    fn where_select() {
        let tokens = vec![
            Token::Select, Token::Ident("col2".into()),Token::Comma, Token::Ident("col1".into()),
            Token::From, Token::Ident("table".to_string()),
            Token::Where, Token::LParen,
            Token::Ident("status".into()), Token::Eq, Token::StringLiteral("active".into()),
            Token::Or,
            Token::Ident("status".into()), Token::NotEq, Token::StringLiteral("banned".into()),
            Token::RParen,
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = SelectStmt {
            table: TableRef::Table("table".to_string()),
            columns: vec![
                ColumnRef::Column { table: None, column: "col2".into() },
                ColumnRef::Column { table: None, column: "col1".into() }
            ],
            where_clause: Some(bin(
                    bin(col("status"), BOp::Eq, str_("active")),
                    BOp::Or,
                    bin(col("status"), BOp::NotEq, str_("banned"))
            ))
        };

        assert_eq!(parser.parse_select().unwrap(), expected);
    }

    #[test]
    fn dotted_select() {
        let tokens = vec![
            Token::Select, Token::Ident("users".into()), Token::Dot, Token::Ident("names".into()),
            Token::As, Token::Ident("u".into()),
            Token::From, Token::Ident("table".into()),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = SelectStmt {
            table: TableRef::Table("table".to_string()),
            columns: vec![
                ColumnRef::Alias { 
                    alias: "u".into(),
                    column: Box::new(ColumnRef::Column {
                        table: Some(TableRef::Table("users".into())),
                        column: "names".into(),
                    }),
                }
            ],
            where_clause: None,
        };

        assert_eq!(parser.parse_select().unwrap(), expected);
    }
}
