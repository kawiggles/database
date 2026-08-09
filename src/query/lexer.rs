use crate::{
    errors::{UserResult, UserErr, QueryErr, QueryResult},
};

use std::{
    str::from_utf8,
};

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Keywords
    Select, From, Where, Insert, Into, Values,
    Create, Table, Copy, Stdin, Stdout, Update,
    And, Or, Not, Null, As,

    // Literals
    Ident(String),
    IntLiteral(i64),
    StringLiteral(String),

    // Operators
    Eq, NotEq, Lt, Gt, LtEq, GtEq,

    // Punctuation
    Comma, Semicolon, LParen, RParen, Dot, Star,

    Eof
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
                tokens.push(Token::Dot);
                start += 1;
            },
            b'*' => { 
                tokens.push(Token::Star);
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
                    tokens.push(scan_int_literal(&mut end, query)?);
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
        "SELECT" => Ok(Token::Select),
        "FROM" => Ok(Token::From),
        "WHERE" => Ok(Token::Where),
        "INSERT" => Ok(Token::Insert),
        "INTO" => Ok(Token::Into),
        "VALUES" => Ok(Token::Values),
        "CREATE" => Ok(Token::Create),
        "TABLE" => Ok(Token::Table),
        "COPY" => Ok(Token::Copy),
        "STDIN" => Ok(Token::Stdin),
        "STDOUT" => Ok(Token::Stdout),
        "UPDATE" => Ok(Token::Update),
        "AND" => Ok(Token::And),
        "OR" => Ok(Token::Or),
        "NOT" => Ok(Token::Not),
        "NULL" => Ok(Token::Null),
        "AS" => Ok(Token::As),
        _ => Ok(Token::Ident(text.to_string())),
    }
}

fn scan_int_literal(end: &mut usize, query: &[u8]) -> QueryResult<Token> {
    let mut digits: Vec<u8> = Vec::new();

    while *end < query.len() && query[*end].is_ascii_digit() {
        digits.push(query[*end]);
        *end += 1;
    }

    let num = from_utf8(&digits)?;
    Ok(Token::IntLiteral(num.parse::<i64>()?))
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
        assert_eq!(token, scan_int_literal(&mut pointer, query.as_bytes()).unwrap());
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
