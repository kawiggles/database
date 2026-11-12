use crate::{
    errors::{QueryErr, QueryResult},
    query::lexer::Token,
    store::relation::schema::Type,
};

#[derive(Debug, PartialEq)]
pub enum Statement {
    Create(Create),
    Drop(Drop),
    Select(Select),
    Insert(Insert),
    Update(Update),
    Delete(Delete),
}

#[derive(Debug, PartialEq)]
struct Create {
    name: String,
    columns: Vec<Definition>,
}

#[derive(Debug, PartialEq)]
struct Drop {
    table: TableRef,
}

#[derive(Debug, PartialEq)]
struct Select {
    projections: Vec<Projection>,
    table: TableRef,
    filter: Option<Expr>,
}

#[derive(Debug, PartialEq)]
struct Insert {
    table: TableRef,
    columns: Vec<ColumnRef>,
    values: Vec<Vec<Expr>>,
}

#[derive(Debug, PartialEq)]
struct Update {
    table: TableRef,
    assignments: Vec<Assignment>,
    where_clause: Option<Expr>,
}

#[derive(Debug, PartialEq)]
struct Delete;

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
pub enum Literal {
    Str(String),
    Int(isize),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, PartialEq)]
pub enum Projection {
    Wildcard,
    QualifiedWildcard(String),
    Expr { expr: Expr, alias: Option<String> },
}

#[derive(Debug, PartialEq)]
pub struct ColumnRef {
    qualifier: Option<String>,
    column: String,
}

#[derive(Debug, PartialEq)]
pub struct TableRef {
    name: String,
    alias: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum BOp { Eq, NotEq, Lt, Gt, LtEq, GtEq, And, Or, Add, Sub, Mul, Div }

#[derive(Debug, PartialEq)]
pub enum UOp { Not, Neg, IsNull }

#[derive(Debug, PartialEq)]
pub struct Assignment {
    column: ColumnRef,
    val: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct Definition {
    name: String,
    ty: Type,
    primary: bool,
    not_null: bool,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// Use if the next token is optional
    fn peek(&self) -> &'a Token {
        &self.tokens.get(self.pos).expect("Advanced past eof parsing tokens")
    }

    /// Use to consume a peeked token
    fn advance(&mut self) -> Token {
        let t = self.tokens.get(self.pos).expect("Advanced past eof parsing tokens").clone();
        self.pos += 1;
        t
    }

    /// Use if a statement has a required token
    fn expect(&mut self, expected: &Token) -> QueryResult<()> {
        let token = self.advance();
        if &token == expected { Ok(()) } else {
            Err(QueryErr::UnexpectedToken {
                found: token,
                expected: format!("{}", expected)
            })
        }
    }

    /// Used specifically to check for and consume an Ident token
    fn expect_ident(&mut self) -> QueryResult<String> {
        match self.advance() {
            Token::Ident(ident) => Ok(ident),
            other => Err(QueryErr::UnexpectedToken {
                found: other.clone(),
                expected: "an identifier".into()
            }),
        }
    }
    
    // TODO: use pos to peek ahead and see if there's wildcard.
    fn try_qualified_wildcard(&mut self) -> QueryResult<Option<Projection>> {
        todo!()
    }

    fn parse_statement(&mut self) -> QueryResult<Statement> {
        match self.peek() {
            Token::Create   => Ok(Statement::Create(self.parse_create()?)),
            Token::Drop     => Ok(Statement::Drop(self.parse_drop()?)),
            Token::Select   => Ok(Statement::Select(self.parse_select()?)),
            Token::Insert   => Ok(Statement::Insert(self.parse_insert()?)),
            Token::Update   => Ok(Statement::Update(self.parse_update()?)),
            Token::Delete   => Ok(Statement::Delete(self.parse_delete()?)),
            other => Err(QueryErr::UnexpectedToken {
                found: other.clone(),
                expected: "CREATE, DROP, SELECT, INSERT, UPDATE, or DELETE".into()
            }),
        }
    }

    fn parse_create(&mut self) -> QueryResult<Create> {
        self.expect(&Token::Create)?;
        self.expect(&Token::Table)?;
        let name = self.expect_ident()?;

        self.expect(&Token::LParen)?;
        let columns = self.parse_definitions()?;
        self.expect(&Token::RParen)?;

        Ok(Create { name, columns })
    }

    fn parse_drop(&mut self) -> QueryResult<Drop> {
        Ok(Drop { table: self.parse_table_ref()? })
    }

    fn parse_select(&mut self) -> QueryResult<Select> {
        self.expect(&Token::Select)?;
        let projections = self.parse_projections()?;
        self.expect(&Token::From)?;
        let from = self.parse_table_ref()?;

        let filter = if self.peek() == &Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Select { projections, from, filter })
    }

    fn parse_insert(&mut self) -> QueryResult<Insert> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;
        let table = self.parse_table_ref()?;

        self.expect(&Token::LParen)?;
        let columns = self.parse_column_list()?;
        self.expect(&Token::RParen)?;

        self.expect(&Token::Values)?;
        let values = self.parse_values()?;

        Ok(Insert { table, columns, values })
    }

    fn parse_update(&mut self) -> QueryResult<Update> {
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
        
        Ok(Update { table, assignments, where_clause })
    }

    fn parse_delete(&mut self) -> QueryResult<Delete> {
        todo!()
    }

    fn parse_definitions(&mut self) -> QueryResult<Vec<Definition>> {
        let mut columns = vec![self.parse_definition()?];

        while self.peek() == &Token::Comma {
            self.advance();
            columns.push(self.parse_definition()?);
        }

        Ok(columns)
    }

    fn parse_definition(&mut self) -> QueryResult<Definition> {
        let name = self.expect_ident()?;

        let ty = match self.expect_ident()?.as_str() {
            "bool"  => Type::Bool,
            "int"   => Type::Int,
            "uint"  => Type::Uint,
            "float" => Type::Float,
            "text"  => Type::Text,
            "blob"  => Type::Blob,
            other   => return Err(QueryErr::UnknownType(other.into()))
        };

        let mut primary = false;
        let mut not_null = false;

        loop {
            match self.peek() {
                Token::Primary => {
                    self.advance();
                    self.expect(&Token::Key)?;
                    primary = true;
                },
                Token::Not => {
                    self.advance();
                    self.expect(&Token::Null)?;
                    not_null = true;
                },
                Token::Comma | Token::RParen => break,
                other => return Err(QueryErr::UnexpectedToken {
                    found: other.clone(),
                    expected: "PRIMARY KEY, NOT NULL, ',', or ')'".into(),
                }),
            }
        }

        Ok(Definition { name, ty, primary, not_null })
    }

    fn parse_table_ref(&mut self) -> QueryResult<TableRef> {
        let name = self.expect_ident()?;
        if self.peek() == &Token::As {
            self.advance();
            let alias = self.expect_ident()?;
            Ok(TableRef { name, alias: Some(alias) })
        } else {
            Ok(TableRef { name, alias: None })
        }
    }

    fn parse_projections(&mut self) -> QueryResult<Vec<Projection>> {
        let mut items = vec![self.parse_projection()?];

        while self.peek() == &Token::Comma {
            self.advance();
            items.push(self.parse_projection()?);
        }

        Ok(items)
    }

    fn parse_projection(&mut self) -> QueryResult<Projection> {
        if self.peek() == &Token::Star {
            self.advance();
            return Ok(Projection::Wildcard)
        }

        // TODO: take this and chuck it into parse primary?
        let mut column = self.expect_ident()?;
        let mut qualifier = None;
        if self.peek() == &Token::Dot {
            self.advance();
            if self.peek() == &Token::Star {
                self.advance();
                return Ok(Projection::QualifiedWildcard(column));
            } else {
                qualifier = Some(column);
                column = self.expect_ident()?;
            }
        }

        let alias = if self.peek() == &Token::As {
            self.advance();
            Some(self.expect_ident()?)
        } else { None };

        Ok(Projection::Expr { expr: Expr::ColumnRef(ColumnRef { qualifier, column }), alias })
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

    fn parse_assignments(&mut self) -> QueryResult<Vec<Assignment>> {
        let mut assignments = vec![self.parse_assignment()?];

        while self.peek() == &Token::Comma {
            self.advance();
            assignments.push(self.parse_assignment()?);
        }

        Ok(assignments)
    }

    fn parse_assignment(&mut self) -> QueryResult<Assignment> {
        // TODO: UPDATE target column parsing
        // wtf does this mean past me???
        let column = self.parse_column_ref()?;
        self.expect(&Token::Eq)?;
        let val = self.parse_expr()?;

        Ok(Assignment {
            column: column,
            val: Box::new(val),
        })
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
            Token::FloatLiteral(f) => Ok(Expr::Literal(Literal::Float(f))),
            Token::StringLiteral(s) => Ok(Expr::Literal(Literal::Str(s))),
            Token::True => Ok(Expr::Literal(Literal::Bool(true))),
            Token::False => Ok(Expr::Literal(Literal::Bool(false))),
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
    
    // TODO: expand for arithmetic
    fn peek_binop(&self) -> Option<BOp> {
        match self.peek() {
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
    // TODO: Multiline processing loop, this will be complicated
    let mut parser = Parser::new(&tokens);
    let stmt = parser.parse_statement()?;
    parser.expect(&Token::Semicolon)?;
    Ok(stmt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::ast::{Expr};

    fn int(n: isize) -> Expr {
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
    fn basic_create_table() {
        let tokens = vec![
            Token::Create, Token::Table, Token::Ident("users".into()),
            Token::LParen,
            Token::Ident("id".into()), Token::Ident("uint".into()), Token::Primary, Token::Key,
            Token::Comma,
            Token::Ident("name".into()), Token::Ident("text".into()), Token::Not, Token::Null,
            Token::RParen, Token::Eof
        ];
        let mut parser = Parser::new(&tokens);

        let expected = Create {
            name: "users".into(),
            columns: vec![
                Definition { name: "id".into(), ty: Type::Uint, primary: true, not_null: false },
                Definition { name: "name".into(), ty: Type::Text, primary: false, not_null: true },
            ],
        };

        assert_eq!(parser.parse_create().unwrap(), expected);
    }

    #[test]
    fn basic_drop_table() {
        let tokens = vec![
            Token::Drop, Token::Table, Token::Ident("users".into()), Token::Eof
        ];
        let mut parser = Parser::new(&tokens);

        let expected = Drop { table: TableRef { name: "users".into(), alias: None } };
        assert_eq!(parser.parse_drop().unwrap(), expected);
    }

    #[test]
    fn basic_select() {
        let tokens = vec![
            Token::Select, Token::Star,
            Token::From, Token::Ident("table".into()),
            Token::Eof,
        ];
        let mut parser = Parser::new(&tokens);

        let expected = Select {
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

        let expected = Select {
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

        let expected = Select {
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

    #[test]
    fn basic_insert() {
        let tokens = vec![
            Token::Insert, Token::Into, Token::Ident("users".into()), Token::LParen,
            Token::Ident("id".into()), Token::Comma, Token::Ident("name".into()), Token::RParen,
            Token::Values, Token::LParen, 
            Token::IntLiteral(1), Token::Comma, Token::StringLiteral("bob".into()),
            Token::RParen, Token::Comma, Token::LParen,
            Token::IntLiteral(2), Token::Comma, Token::StringLiteral("bill".into()),
            Token::RParen, Token::Eof
        ];
        let mut parser = Parser::new(&tokens);

        let expected = Insert {
            table: TableRef::Table("users".into()),
            columns: vec![
                ColumnRef::Column { table: None, column: "id".into() },
                ColumnRef::Column { table: None, column: "name".into() },
            ],
            values: vec![
                vec![int(1), str_("bob")],
                vec![int(2), str_("bill")],
            ],
        };

        assert_eq!(parser.parse_insert().unwrap(), expected);
    }

    #[test]
    fn basic_update() {
        let tokens = vec![
            Token::Update, Token::Ident("users".into()),
            Token::Set, Token::Ident("name".into()), Token::Eq, Token::StringLiteral("Bob".into()),
            Token::Where, Token::Ident("id".into()), Token::Eq, Token::IntLiteral(1),
            Token::Eof
        ];
        let mut parser = Parser::new(&tokens);

        let expected = Update {
            table: TableRef::Table("users".into()),
            assignments: vec![Assignment {
                column: ColumnRef::Column { table: None, column: "name".into() },
                val: Box::new(str_("Bob")),
            }],
            where_clause: Some(bin(col("id"), BOp::Eq, int(1))),
        };

        assert_eq!(parser.parse_update().unwrap(), expected);
    }
}
