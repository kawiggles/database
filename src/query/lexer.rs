use crate::{
    errors::{UserResult, UserErr, QueryErr, QueryResult},
};

use std::{
    str::from_utf8,
    fmt::Display,
};

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Statements
    Create, Drop, Select, Insert, Update, Delete,

    // Keywords
    Table, Where, From, Into, Values, Set, As, Is,
    And, Or, Not, Null, True, False, Primary, Key,

    // Literals
    Ident(String),
    IntLiteral(isize),
    FloatLiteral(f64),
    StringLiteral(String),

    // Operators
    Eq, NotEq, Lt, Gt, LtEq, GtEq, Plus, Minus, Slash,

    // Punctuation
    Comma, Semicolon, LParen, RParen, Dot, Star,

    Eof
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Create           => write!(f, "CREATE"),
            Token::Drop             => write!(f, "DROP"),
            Token::Select           => write!(f, "SELECT"),
            Token::Insert           => write!(f, "INSERT"),
            Token::Update           => write!(f, "UPDATE"),
            Token::Delete           => write!(f, "DELETE"),

            Token::Table            => write!(f, "TABLE"),
            Token::Where            => write!(f, "WHERE"),
            Token::From             => write!(f, "FROM"),
            Token::Into             => write!(f, "INTO"),
            Token::Values           => write!(f, "VALUES"),
            Token::Set              => write!(f, "SET"),
            Token::As               => write!(f, "AS"),
            Token::Is               => write!(f, "IS"),
            Token::And              => write!(f, "AND"),
            Token::Or               => write!(f, "OR"),
            Token::Not              => write!(f, "NOT"),
            Token::Null             => write!(f, "NULL"),
            Token::True             => write!(f, "TRUE"),
            Token::False            => write!(f, "FALSE"),
            Token::Primary          => write!(f, "PRIMARY"),
            Token::Key              => write!(f, "KEY"),

            Token::Ident(i)         => write!(f, "ident<{}>", i),
            Token::IntLiteral(i)    => write!(f, "int<{}>", i),
            Token::FloatLiteral(fl) => write!(f, "float<{}>", fl),
            Token::StringLiteral(s) => write!(f, "string<{}>", s),

            Token::Eq               => write!(f, "="),
            Token::NotEq            => write!(f, "!="),
            Token::Lt               => write!(f, "<"),
            Token::Gt               => write!(f, ">"),
            Token::LtEq             => write!(f, "<="),
            Token::GtEq             => write!(f, ">="),
            Token::Plus             => write!(f, "+"),
            Token::Minus            => write!(f, "-"),
            Token::Slash            => write!(f, "/"),

            Token::Comma            => write!(f, ","),
            Token::Semicolon        => write!(f, ";"),
            Token::LParen           => write!(f, "("),
            Token::RParen           => write!(f, ")"),
            Token::Dot              => write!(f, "."),
            Token::Star             => write!(f, "*"),
            Token::Eof              => write!(f, "EOF"),
        }
    }
}

pub fn lexerize(query: &[u8]) -> UserResult<Vec<Token>> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut start: usize = 0;
    let mut end: usize;

    loop {
        if start >= query.len() {
            tokens.push(Token::Eof);
            break;
        }

        end = start;

        if query[start] == b' ' {
            start += 1;
            continue;
        }

        match &query[start] {
            b',' => { 
                tokens.push(Token::Comma);
                start += 1;
            },
            b';' => { 
                tokens.push(Token::Semicolon);
                start += 1;
            },
            b'(' => { 
                tokens.push(Token::LParen);
                start += 1;
            },
            b')' => { 
                tokens.push(Token::RParen);
                start += 1;
            },
            b'.' => { 
                if start + 1 < query.len() && query[start+1].is_ascii_digit() {
                    tokens.push(scan_num_literal(&mut end, query)?);
                    start = end;
                } else {
                    tokens.push(Token::Dot);
                    start += 1;
                }
            },
            b'*' => { 
                tokens.push(Token::Star);
                start += 1;
            },
            b'+' => {
                tokens.push(Token::Plus);
                start += 1;
            },
            b'-' => {
                tokens.push(Token::Minus);
                start += 1;
            },
            b'/' => {
                tokens.push(Token::Slash);
                start += 1;
            },
            b'!' => { 
                tokens.push(Token::NotEq); // This will break if "!" becomes a sql operator
                start += 1;
            },
            b'=' => { 
                tokens.push(Token::Eq);
                start += 1;
            },
            b'<' => {
                if start + 1 < query.len() && query[start+1] == b'=' {
                    tokens.push(Token::LtEq);
                    start += 2;
                } else {
                    tokens.push(Token::Lt);
                    start += 1;
                }
            },
            b'>' => {
                if start + 1 < query.len() && query[start+1] == b'=' {
                    tokens.push(Token::GtEq);
                    start += 2;
                } else {
                    tokens.push(Token::Gt);
                    start += 1;
                }
            },
            b'\'' => {
                tokens.push(scan_string_literal(&mut end, query)?);
                start = end;
            },
            _ => {
                if query[start].is_ascii_alphabetic() || query[start] == b'_' {
                    tokens.push(scan_ident_or_keyword(&mut end, query)?);
                    start = end;
                } else if query[start].is_ascii_digit() {
                    tokens.push(scan_num_literal(&mut end, query)?);
                    start = end;
                } else {
                    return Err(UserErr::BadQuery(QueryErr::NonAsciiChar{
                        pos: start,
                        byte: query[start]
                    }));
                }
            }
        }
    }

    Ok(tokens)
}

fn scan_string_literal(end: &mut usize, query: &[u8]) -> QueryResult<Token> {
    let mut literal: Vec<u8> = Vec::new();

    *end += 1; // so that the first byte read isn't the initiating \'
    loop {
        if *end >= query.len() {
            return Err(QueryErr::StrLiteralNoClose);
        }

        match query[*end] {
            b'\'' => {
                if *end + 1 < query.len() && query[*end+1] == b'\'' {
                    *end += 1;
                    literal.push(b'\'');
                } else {
                    *end += 1;
                    break;
                }
            },
            b'\\' => {
                if query[*end+1] == b'\'' {
                    *end += 1;
                    literal.push(b'\'');
                } else {
                    literal.push(b'\\');
                }
            },
            _ => literal.push(query[*end]),
        }

        *end += 1;
    }

    Ok(Token::StringLiteral(from_utf8(&literal)?.to_string())) // should never panic
}

fn scan_ident_or_keyword(end: &mut usize, query: &[u8]) -> QueryResult<Token> {
    let mut word: Vec<u8> = Vec::new();

    while *end < query.len() && (query[*end].is_ascii_alphanumeric() || query[*end] == b'_') {
        word.push(query[*end]);
        *end += 1;
    }

    let text = from_utf8(&word)?;
    match text.to_ascii_uppercase().as_str() {
        "CREATE"    => Ok(Token::Create),
        "DROP"      => Ok(Token::Drop),
        "SELECT"    => Ok(Token::Select),
        "INSERT"    => Ok(Token::Insert),
        "UPDATE"    => Ok(Token::Update),
        "DELETE"    => Ok(Token::Delete),
        "TABLE"     => Ok(Token::Table),
        "WHERE"     => Ok(Token::Where),
        "FROM"      => Ok(Token::From),
        "INTO"      => Ok(Token::Into),
        "VALUES"    => Ok(Token::Values),
        "SET"       => Ok(Token::Set),
        "AS"        => Ok(Token::As),
        "IS"        => Ok(Token::Is),
        "AND"       => Ok(Token::And),
        "OR"        => Ok(Token::Or),
        "NOT"       => Ok(Token::Not),
        "NULL"      => Ok(Token::Null),
        "TRUE"      => Ok(Token::True),
        "FALSE"     => Ok(Token::False),
        "PRIMARY"   => Ok(Token::Primary),
        "KEY"       => Ok(Token::Key),
        _ => Ok(Token::Ident(text.to_string())),
    }
}

fn scan_num_literal(end: &mut usize, query: &[u8]) -> QueryResult<Token> {
    let mut digits: Vec<u8> = Vec::new();
    let mut float = false;

    while *end < query.len() {
        if query[*end].is_ascii_digit() {
            digits.push(query[*end]);
            *end += 1;
        } else if query[*end] == b'.' {
            float = true;
            digits.push(query[*end]);
            *end += 1;
        } else {
            break;
        }
    }

    let num = from_utf8(&digits)?;
    if float {
        Ok(Token::FloatLiteral(num.parse::<f64>()?))
    } else {
        Ok(Token::IntLiteral(num.parse::<isize>()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_string_literal() {
        let query = "'this is \\\'a\\\' string'";
        let mut pointer = 0;
        let token = Token::StringLiteral("this is \'a\' string".to_string());
        assert_eq!(token, scan_string_literal(&mut pointer, query.as_bytes()).unwrap());
    }

    #[test]
    fn lex_int_literal() {
        let query = "12345";
        let mut pointer = 0;
        let token = Token::IntLiteral(12345);
        assert_eq!(token, scan_num_literal(&mut pointer, query.as_bytes()).unwrap());
    }

    #[test]
    fn lex_float_literal() {
        let query = "123.456";
        let mut pointer = 0;
        let token = Token::FloatLiteral(123.456);
        assert_eq!(token, scan_num_literal(&mut pointer, query.as_bytes()).unwrap());
    }

    #[test]
    fn lex_fraction_literal() {
        let query = ".456";
        let expected = vec![Token::FloatLiteral(0.456)];
        assert_eq!(expected, lexerize(query.as_bytes()).unwrap());
    }

    #[test]
    fn lex_ident_or_keyword() {
        let query = "SELECT";
        let mut pointer = 0;
        let token = Token::Select;
        assert_eq!(token, scan_ident_or_keyword(&mut pointer, query.as_bytes()).unwrap());
        let query = "FROM";
        let mut pointer = 0;
        let token = Token::From;
        assert_eq!(token, scan_ident_or_keyword(&mut pointer, query.as_bytes()).unwrap());
        let query = "some_ident";
        let mut pointer = 0;
        let token = Token::Ident("some_ident".to_string());
        assert_eq!(token, scan_ident_or_keyword(&mut pointer, query.as_bytes()).unwrap());
    }

    #[test]
    fn lex_query() {
        let query = "SELECT * FROM yo WHERE val >= 35 AND other = 'string';";
        let tokens: Vec<Token> = vec![
            Token::Select,
            Token::Star,
            Token::From,
            Token::Ident("yo".to_string()),
            Token::Where,
            Token::Ident("val".to_string()),
            Token::GtEq,
            Token::IntLiteral(35),
            Token::And,
            Token::Ident("other".to_string()),
            Token::Eq,
            Token::StringLiteral("string".to_string()),
            Token::Semicolon,
            Token::Eof,
        ];
        assert_eq!(tokens, lexerize(query.as_bytes()).unwrap())
    }
}
