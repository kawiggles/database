use crate::{
    errors::{UserResult, UserErr},
};

use std::{
    str::from_utf8,
};

#[derive(Debug, PartialEq, Clone)]
enum Token {
    // Keywords
    Select, From, Where, Insert, Into, Values,
    Create, Table, Copy, Stdin, Stdout,
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

        match &query[end] {
            b'\'' => {
                tokens.push(scan_string_literal(&mut end, query));
                start = end;
            },
            b'\n' => break,
            _ => return Err(UserErr::BadQuery),
        }
    }

    Ok(tokens)
}

fn scan_string_literal(end: &mut usize, query: &[u8]) -> Token {
    let mut literal: Vec<u8> = Vec::new();

    *end += 1;
    while query[*end] != b'\'' {
        if query[*end] == b'\\' {
            *end += 1;
            if query[*end] == b'\'' {
                literal.push(b'\'');
            }
        } else {
            literal.push(query[*end]);
        }
        *end += 1;
    }

    Token::StringLiteral(from_utf8(&literal).unwrap().to_string()) // should never panic
}
