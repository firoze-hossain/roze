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
        // Update line/column based on the character we're leaving behind
        // (if any), *before* fetching the next one. Doing this the other
        // way around -- checking the newly-fetched character for '\n' --
        // resets the column one character too early, making the first
        // real character after every newline report the wrong column.
        if let Some(c) = self.current_char {
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        self.current_char = self.chars.next();
        self.position += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_keywords_and_identifiers() {
        let tokens = tokenize("func main");
        assert_eq!(tokens[0].token, Token::Func);
        assert_eq!(tokens[1].token, Token::Identifier("main".to_string()));
        assert_eq!(tokens[2].token, Token::EOF);
    }

    #[test]
    fn tokenizes_two_character_operators_greedily() {
        let tokens = tokenize("== != <= >= -> && ||");
        assert_eq!(tokens[0].token, Token::EqualsEquals);
        assert_eq!(tokens[1].token, Token::NotEquals);
        assert_eq!(tokens[2].token, Token::LessEqual);
        assert_eq!(tokens[3].token, Token::GreaterEqual);
        assert_eq!(tokens[4].token, Token::Arrow);
        assert_eq!(tokens[5].token, Token::And);
        assert_eq!(tokens[6].token, Token::Or);
    }

    #[test]
    fn skips_line_and_block_comments() {
        let tokens = tokenize("1 // a comment\n2 /* block */ 3");
        let non_eof: Vec<&Token> = tokens.iter().map(|t| &t.token).filter(|t| **t != Token::EOF).collect();
        assert_eq!(non_eof, vec![
            &Token::Number("1".to_string()),
            &Token::Number("2".to_string()),
            &Token::Number("3".to_string()),
        ]);
    }

    #[test]
    fn resolves_string_escapes() {
        let tokens = tokenize(r#""a\nb\tc\"d""#);
        match &tokens[0].token {
            Token::String(s) => assert_eq!(s, "a\nb\tc\"d"),
            other => panic!("expected a String token, got {:?}", other),
        }
    }

    #[test]
    fn unrecognized_character_becomes_error_token() {
        let tokens = tokenize("@");
        assert_eq!(tokens[0].token, Token::Error);
    }

    /// The very first character of a file must be column 1, not 2. The
    /// lexer's constructor primes `current_char` with an initial
    /// `advance()` call; a previous version of that call incremented
    /// `column` before ever reading the first character, so this exact
    /// case silently reported column 2 for years before anything ever
    /// rendered a column number to a person.
    #[test]
    fn first_character_of_file_is_column_one() {
        let tokens = tokenize("x");
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].column, 1);
    }

    /// The first character after *any* newline must be column 1. A
    /// previous version of `advance()` decided whether to reset the
    /// column by checking the character it had just arrived at, rather
    /// than the one it was leaving behind -- so the reset happened one
    /// character too early, and the first real character of every line
    /// after the first was reported one column too high.
    #[test]
    fn first_character_after_newline_is_column_one() {
        let tokens = tokenize("x\n@");
        // tokens[0] = 'x' (line 1), tokens[1] = '@' (line 2)
        assert_eq!(tokens[1].line, 2);
        assert_eq!(tokens[1].column, 1);
    }

    #[test]
    fn column_tracks_correctly_mid_line() {
        let tokens = tokenize("    let x = 5 @ 3;");
        // '@' is the 15th character (1-indexed): 4 spaces + "let x = 5 " (10 chars) = 14, so '@' is at column 15.
        let at_token = tokens.iter().find(|t| t.token == Token::Error).expect("expected an Error token for '@'");
        assert_eq!(at_token.column, 15);
    }

    #[test]
    fn line_increments_once_per_newline() {
        let tokens = tokenize("a\nb\nc");
        let lines: Vec<usize> = tokens.iter().filter(|t| t.token != Token::EOF).map(|t| t.line).collect();
        assert_eq!(lines, vec![1, 2, 3]);
    }
}