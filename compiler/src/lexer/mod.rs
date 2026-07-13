pub mod token;

use token::{Token, TokenWithLocation};
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: &'a str,
    chars: Peekable<Chars<'a>>,
    position: usize,
    line: usize,
    column: usize,
    current_char: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Self {
            input,
            chars: input.chars().peekable(),
            position: 0,
            line: 1,
            column: 1,
            current_char: None,
        };
        lexer.advance();
        lexer
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
        if let Some(c) = self.current_char {
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.position += 1;
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else if c == '/' && self.peek_char() == Some('/') {
                // Single-line comment - skip until newline or EOF
                while let Some(ch) = self.current_char {
                    if ch == '\n' {
                        break;
                    }
                    self.advance();
                }
                // Don't advance past newline here - let the whitespace handler do it
            } else if c == '/' && self.peek_char() == Some('*') {
                // Multi-line comment - skip until */
                self.advance(); // Skip '/'
                self.advance(); // Skip '*'
                while let Some(ch) = self.current_char {
                    if ch == '*' && self.peek_char() == Some('/') {
                        self.advance(); // Skip '*'
                        self.advance(); // Skip '/'
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> String {
        let mut number = String::new();
        while let Some(c) = self.current_char {
            if c.is_digit(10) || c == '.' {
                number.push(c);
                self.advance();
            } else {
                break;
            }
        }
        number
    }

    fn read_identifier(&mut self) -> String {
        let mut ident = String::new();
        while let Some(c) = self.current_char {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }
        ident
    }

    fn read_string(&mut self) -> String {
        let mut string = String::new();
        self.advance(); // Skip opening quote

        while let Some(c) = self.current_char {
            if c == '"' {
                self.advance();
                break;
            } else if c == '\\' {
                self.advance();
                if let Some(escaped) = self.current_char {
                    match escaped {
                        'n' => string.push('\n'),
                        't' => string.push('\t'),
                        'r' => string.push('\r'),
                        '"' => string.push('"'),
                        '\\' => string.push('\\'),
                        _ => string.push(escaped),
                    }
                    self.advance();
                }
            } else {
                string.push(c);
                self.advance();
            }
        }
        string
    }

    pub fn next_token(&mut self) -> TokenWithLocation {
        self.skip_whitespace_and_comments();

        let line = self.line;
        let column = self.column;

        if let Some(c) = self.current_char {
            let token = match c {
                '(' => { self.advance(); Token::LeftParen }
                ')' => { self.advance(); Token::RightParen }
                '{' => { self.advance(); Token::LeftBrace }
                '}' => { self.advance(); Token::RightBrace }
                ',' => { self.advance(); Token::Comma }
                '.' => { self.advance(); Token::Dot }
                ';' => { self.advance(); Token::Semicolon }
                ':' => { self.advance(); Token::Colon }

                '=' => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        Token::EqualsEquals
                    } else {
                        Token::Equals
                    }
                }
                '!' => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        Token::NotEquals
                    } else {
                        Token::Not
                    }
                }
                '<' => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        Token::LessEqual
                    } else {
                        Token::LessThan
                    }
                }
                '>' => {
                    self.advance();
                    if self.current_char == Some('=') {
                        self.advance();
                        Token::GreaterEqual
                    } else {
                        Token::GreaterThan
                    }
                }
                '+' => { self.advance(); Token::Plus }
                '-' => {
                    self.advance();
                    if self.current_char == Some('>') {
                        self.advance();
                        Token::Arrow
                    } else {
                        Token::Minus
                    }
                }
                '*' => { self.advance(); Token::Star }
                '/' => { self.advance(); Token::Slash }
                '&' => {
                    self.advance();
                    if self.current_char == Some('&') {
                        self.advance();
                        Token::And
                    } else {
                        Token::Error
                    }
                }
                '|' => {
                    self.advance();
                    if self.current_char == Some('|') {
                        self.advance();
                        Token::Or
                    } else {
                        Token::Error
                    }
                }
                '"' => {
                    let string = self.read_string();
                    Token::String(string)
                }
                _ => {
                    if c.is_digit(10) {
                        let number = self.read_number();
                        Token::Number(number)
                    } else if c.is_alphabetic() || c == '_' {
                        let ident = self.read_identifier();
                        match ident.as_str() {
                            "let" => Token::Let,
                            "mut" => Token::Mut,
                            "func" => Token::Func,
                            "return" => Token::Return,
                            "if" => Token::If,
                            "else" => Token::Else,
                            "for" => Token::For,
                            "while" => Token::While,
                            "class" => Token::Class,
                            "new" => Token::New,
                            "import" => Token::Import,
                            "true" => Token::True,
                            "false" => Token::False,
                            "null" => Token::Null,
                            _ => Token::Identifier(ident),
                        }
                    } else {
                        self.advance();
                        Token::Error
                    }
                }
            };

            TokenWithLocation::new(token, line, column)
        } else {
            TokenWithLocation::new(Token::EOF, line, column)
        }
    }

    pub fn tokenize(&mut self) -> Vec<TokenWithLocation> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            tokens.push(token.clone());
            if let Token::EOF = token.token {
                break;
            }
        }
        tokens
    }
}

pub fn tokenize(input: &str) -> Vec<TokenWithLocation> {
    let mut lexer = Lexer::new(input);
    lexer.tokenize()
}