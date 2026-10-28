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
    Enum,
    Match,
    Trait,
    Impl,
    This,
    Extern,
    Unsafe,
    Raw,
    Type,
    Where,
    Comptime,
    Async,
    Await,
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
    Percent,    // %
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
    Question,   // ?

    // Delimiters
    LeftParen,   // (
    RightParen,  // )
    LeftBrace,   // {
    RightBrace,  // }
    LeftBracket, // [
    RightBracket,// ]
    Comma,       // ,
    Dot,         // .
    DotDot,      // ..
    DotDotEquals,// ..=
    Semicolon,   // ;
    Colon,       // :
    ColonColon,  // ::
    Arrow,       // ->
    FatArrow,    // =>
    Underscore,  // _
    Pipe,        // |

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
            Token::Enum => write!(f, "enum"),
            Token::Match => write!(f, "match"),
            Token::Trait => write!(f, "trait"),
            Token::Impl => write!(f, "impl"),
            Token::This => write!(f, "this"),
            Token::Extern => write!(f, "extern"),
            Token::Unsafe => write!(f, "unsafe"),
            Token::Raw => write!(f, "raw"),
            Token::Type => write!(f, "type"),
            Token::Where => write!(f, "where"),
            Token::Comptime => write!(f, "comptime"),
            Token::Async => write!(f, "async"),
            Token::Await => write!(f, "await"),
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
            Token::Percent => write!(f, "%"),
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
            Token::Question => write!(f, "?"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::LeftBrace => write!(f, "{{"),
            Token::RightBrace => write!(f, "}}"),
            Token::Comma => write!(f, ","),
            Token::LeftBracket => write!(f, "["),
            Token::RightBracket => write!(f, "]"),
            Token::Dot => write!(f, "."),
            Token::DotDot => write!(f, ".."),
            Token::DotDotEquals => write!(f, "..="),
            Token::Semicolon => write!(f, ";"),
            Token::Colon => write!(f, ":"),
            Token::ColonColon => write!(f, "::"),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::Underscore => write!(f, "_"),
            Token::Pipe => write!(f, "|"),
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