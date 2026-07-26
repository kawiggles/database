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
