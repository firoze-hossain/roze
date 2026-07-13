// compiler/src/lexer/token.rs
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Let,
    Mut,
    Func,
    Return,
    If,
    Else,
    For,
    While,
    Class,
    New,
    Import,
    True,
    False,
    Null,

    // Literals
    Identifier(String),
    String(String),
    Number(String),

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Equals,     // =
    EqualsEquals, // ==
    NotEquals,  // !=
    LessThan,   // <
    GreaterThan, // >
    LessEqual,  // <=
    GreaterEqual, // >=
    And,        // &&
    Or,         // ||
    Not,        // !

    // Delimiters
    LeftParen,   // (
    RightParen,  // )
    LeftBrace,   // {
    RightBrace,  // }
    Comma,       // ,
    Dot,         // .
    Semicolon,   // ;
    Colon,       // :
    Arrow,       // ->

    // Special
    Newline,
    EOF,
    Error,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::Func => write!(f, "func"),
            Token::Return => write!(f, "return"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::For => write!(f, "for"),
            Token::While => write!(f, "while"),
            Token::Class => write!(f, "class"),
            Token::New => write!(f, "new"),
            Token::Import => write!(f, "import"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Null => write!(f, "null"),
            Token::Identifier(id) => write!(f, "{}", id),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Number(n) => write!(f, "{}", n),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Equals => write!(f, "="),
            Token::EqualsEquals => write!(f, "=="),
            Token::NotEquals => write!(f, "!="),
            Token::LessThan => write!(f, "<"),
            Token::GreaterThan => write!(f, ">"),
            Token::LessEqual => write!(f, "<="),
            Token::GreaterEqual => write!(f, ">="),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::Not => write!(f, "!"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::LeftBrace => write!(f, "{{"),
            Token::RightBrace => write!(f, "}}"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::Semicolon => write!(f, ";"),
            Token::Colon => write!(f, ":"),
            Token::Arrow => write!(f, "->"),
            Token::Newline => write!(f, "\\n"),
            Token::EOF => write!(f, "EOF"),
            Token::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenWithLocation {
    pub token: Token,
    pub line: usize,
    pub column: usize,
}

impl TokenWithLocation {
    pub fn new(token: Token, line: usize, column: usize) -> Self {
        Self { token, line, column }
    }
}