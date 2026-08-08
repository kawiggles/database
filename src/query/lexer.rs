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
    let mut end: usize = 0;

    loop {
        if query[start] == b' ' {
            start += 1;
            continue;
        }

        match &query[start].to_ascii_lowercase() {
            b',' => tokens.push(Token::Comma),
            b';' => tokens.push(Token::Semicolon),
            b'(' => tokens.push(Token::LParen),
            b')' => tokens.push(Token::RParen),
            b'.' => tokens.push(Token::Dot),
            b'*' => tokens.push(Token::Star),
            b'!' => tokens.push(Token::NotEq), // This will break if "!" becomes a sql operator
            b'=' => tokens.push(Token::Eq),
            b'<' => {
                if query[start+1] == b'=' {
                    tokens.push(Token::LtEq);
                    start += 1;
                } else {
                    tokens.push(Token::Lt);
                }
            },
            b'>' => {
                if query[start+1] == b'=' {
                    tokens.push(Token::GtEq);
                    start += 1;
                } else {
                    tokens.push(Token::Gt);
                }
            },
            b'\'' => {
                tokens.push(scan_string_literal(&mut end, query)?);
                start = end;
            },
            b'\n' => {
                tokens.push(Token::Eof);
                break; // End of stream
            }, 
            _ => {
                if query[start].is_ascii_alphabetic() || query[start] == b'_' {
                    tokens.push(scan_ident_or_keyword(&mut end, query));
                    start = end;
                } else if query[start].is_ascii_digit() {
                    tokens.push(scan_int_literal(&mut end, query));
                    start = end;
                } else {
                    return Err(UserErr::BadQuery(QueryErr::NonAsciiChar{
                        pos: start,
                        byte: query[start]
                    }));
                }
            }
        }

        start += 1;
        end = start;
    }

    Ok(tokens)
}

fn scan_string_literal(end: &mut usize, query: &[u8]) -> QueryResult<Token> {
    let mut literal: Vec<u8> = Vec::new();

    *end += 1; // so that the first byte read isn't the initiating \'
    loop {
        if *end > query.len() {
            return Err(QueryErr::StrLiteralNoClose(*end));
        }

        match query[*end] {
            b'\'' => {
                if query[*end+1] == b'\'' {
                    *end += 1;
                    literal.push(b'\'');
                } else {
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

fn scan_ident_or_keyword(end: &mut usize, query: &[u8]) -> Token {
    todo!();
}

fn scan_int_literal(end: &mut usize, query: &[u8]) -> Token {
    let mut digits: Vec<u8> = Vec::new();

    while query[*end].is_ascii_digit() {
        digits.push(query[*end]);
    }

    let num = digits.iter()
        .fold(0, |acc, &digit| acc * 10 + digit as i64);

    Token::IntLiteral(num)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_string_literal() {
        let query = "'this is \\\'a\\\' string'";
        let mut pointer = 0;
        let token = Token::StringLiteral("this is \'a\' string".to_string());
        assert_eq!(token, scan_string_literal(&mut pointer, query.as_bytes()).unwrap())
    }

    #[test]
    fn lex_int_literal() {
        let query = "12345";
        let mut pointer = 0;
        let token = Token::IntLiteral(12345);
        assert_eq!(token, scan_int_literal(&mut pointer, query.as_bytes()))
    }

    #[test]
    fn lex_ident_or_keyword() {
    }

    #[test]
    fn lex_query() {
    }
}
