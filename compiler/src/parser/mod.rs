pub mod ast;

use crate::error::RozeError;
use crate::lexer::token::{Token, TokenWithLocation};
use ast::*;
use anyhow::Result;

/// The display width of a token, used to size the `^^^` underline in error
/// reports so it spans the actual offending token instead of a single
/// character. `Token` already implements `Display` with the exact text a
/// person would see in their source, so this just measures that.
fn token_len(token: &Token) -> usize {
    token.to_string().chars().count().max(1)
}

pub struct Parser {
    tokens: Vec<TokenWithLocation>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<TokenWithLocation>) -> Self {
        Self { tokens, position: 0 }
    }

    fn current(&self) -> &TokenWithLocation {
        &self.tokens[self.position]
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn check(&self, token_type: &Token) -> bool {
        std::mem::discriminant(&self.current().token) == std::mem::discriminant(token_type)
    }

    fn match_token(&mut self, token_type: &Token) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Builds a parse error pointing at whatever token is currently
    /// unconsumed, with the underline sized to that token's width.
    fn error(&self, message: impl Into<String>) -> anyhow::Error {
        let tok = self.current();
        RozeError::parser(message, tok.line, tok.column)
            .with_length(token_len(&tok.token))
            .into()
    }

    /// Same as `error`, but for a specific (already-consumed) token,
    /// needed when the problem is with a token we've already advanced
    /// past rather than the current one.
    fn error_at(&self, message: impl Into<String>, tok: &TokenWithLocation) -> anyhow::Error {
        RozeError::parser(message, tok.line, tok.column)
            .with_length(token_len(&tok.token))
            .into()
    }

    fn expect(&mut self, token_type: Token, message: &str) -> Result<TokenWithLocation> {
        if self.position >= self.tokens.len() {
            let (line, column) = self.tokens.last().map(|t| (t.line, t.column)).unwrap_or((1, 1));
            return Err(RozeError::parser(format!("Unexpected end of file: {}", message), line, column).into());
        }

        let current_token = self.current().clone();
        // Compare by type, not by value
        if std::mem::discriminant(&current_token.token) == std::mem::discriminant(&token_type) {
            self.advance();
            Ok(current_token)
        } else {
            Err(self.error(format!("{}: expected {}, found {}", message, token_type, current_token.token)))
        }
    }

    pub fn parse(&mut self) -> Result<Program> {
        let mut program = Program::new();

        while !self.check(&Token::EOF) {
            // Skip newlines
            while self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::EOF) {
                break;
            }

            let stmt = self.parse_statement()?;
            program.statements.push(stmt);
        }

        Ok(program)
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);

        match &self.current().token {
            Token::Import => self.parse_import(),
            Token::Func => self.parse_function(),
            Token::Let => self.parse_let(),
            Token::Return => self.parse_return(),
            Token::LeftBrace => self.parse_block(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Identifier(name) if matches!(
                self.tokens.get(self.position + 1).map(|t| &t.token),
                Some(Token::Equals)
            ) => {
                let name = name.clone();
                self.advance(); // Skip identifier
                self.advance(); // Skip '='
                while self.check(&Token::Newline) {
                    self.advance();
                }
                let value = self.parse_expression()?;
                if self.match_token(&Token::Semicolon) {
                    // Semicolon consumed
                }
                Ok(Statement::Assign {
                    name,
                    value: Box::new(value),
                    location,
                })
            }
            _ => {
                // Parse as expression statement
                let expr = self.parse_expression()?;

                // Optional semicolon
                if self.match_token(&Token::Semicolon) {
                    // Semicolon consumed
                }

                Ok(Statement::Expression {
                    expr: Box::new(expr),
                    location,
                })
            }
        }
    }

    fn parse_import(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);
        self.advance(); // Skip 'import'

        let path_token = self.expect(Token::String(String::new()), "Expected import path")?;
        let path = match path_token.token {
            Token::String(s) => s,
            _ => return Err(self.error_at("Invalid import path", &path_token)),
        };

        Ok(Statement::Import { path, location })
    }

    fn parse_function(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);

        // Skip 'func' token
        if self.check(&Token::Func) {
            self.advance();
        } else {
            return Err(self.error("Expected 'func' keyword"));
        }

        // Skip any whitespace/newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Use check for identifier type instead of expect
        if !self.check(&Token::Identifier(String::new())) {
            return Err(self.error(format!("Expected a function name, found {}", self.current().token))
                .context_hint("function declarations look like: func name(...) { ... }"));
        }

        let name_token = self.current().clone();
        let name = match &name_token.token {
            Token::Identifier(n) => n.clone(),
            _ => return Err(self.error_at("Invalid function name", &name_token)),
        };
        self.advance();

        // Skip newlines before parenthesis
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Expect opening parenthesis
        if !self.check(&Token::LeftParen) {
            return Err(self.error("Expected '(' after function name"));
        }
        self.advance();

        let mut params = Vec::new();

        // Parse parameters
        while !self.check(&Token::RightParen) && !self.check(&Token::EOF) {
            // Skip newlines
            while self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::RightParen) {
                break;
            }

            // Get parameter name
            if !self.check(&Token::Identifier(String::new())) {
                return Err(self.error("Expected a parameter name"));
            }

            let param_token = self.current().clone();
            let param_name = match &param_token.token {
                Token::Identifier(n) => n.clone(),
                _ => return Err(self.error_at("Invalid parameter name", &param_token)),
            };
            self.advance();

            // Optional type annotation
            let param_type = if self.check(&Token::Colon) {
                self.advance();
                if !self.check(&Token::Identifier(String::new())) {
                    return Err(self.error("Expected a type name after ':'"));
                }
                let type_token = self.current().clone();
                let type_name = match &type_token.token {
                    Token::Identifier(t) => Some(t.clone()),
                    _ => None,
                };
                self.advance();
                type_name
            } else {
                None
            };

            params.push(FunctionParam {
                name: param_name,
                type_name: param_type,
            });

            // Skip newlines before comma
            while self.check(&Token::Newline) {
                self.advance();
            }

            if !self.check(&Token::Comma) {
                break;
            }
            self.advance(); // Skip comma
        }

        // Expect closing parenthesis
        if !self.check(&Token::RightParen) {
            return Err(self.error("Expected ')' after parameters"));
        }
        self.advance();

        // Skip newlines before an optional return type / body
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Optional return type: '-> TypeName'
        let return_type = if self.check(&Token::Arrow) {
            self.advance(); // Skip '->'
            while self.check(&Token::Newline) {
                self.advance();
            }
            if !self.check(&Token::Identifier(String::new())) {
                return Err(self.error("Expected a return type after '->'"));
            }
            let type_token = self.current().clone();
            let type_name = match &type_token.token {
                Token::Identifier(t) => Some(t.clone()),
                _ => None,
            };
            self.advance();
            type_name
        } else {
            None
        };

        // Skip newlines before body
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Parse body (should be a block)
        let body = self.parse_statement()?;

        Ok(Statement::Function {
            name,
            params,
            return_type,
            body: Box::new(body),
            location,
        })
    }

    fn parse_let(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);
        self.advance(); // Skip 'let'

        // Skip newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        if !self.check(&Token::Identifier(String::new())) {
            return Err(self.error("Expected a variable name after 'let'"));
        }

        let name_token = self.current().clone();
        let name = match name_token.token.clone() {
            Token::Identifier(n) => n,
            _ => return Err(self.error_at("Invalid variable name", &name_token)),
        };
        self.advance();

        // Skip newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        if !self.check(&Token::Equals) {
            return Err(self.error("Expected '=' after variable name")
                .context_hint(format!("did you mean: let {} = ...;", name)));
        }
        self.advance();

        // Skip newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        let value = self.parse_expression()?;

        if self.check(&Token::Semicolon) {
            self.advance(); // Optional semicolon
        }

        Ok(Statement::Let {
            name,
            value: Box::new(value),
            location,
        })
    }

    fn parse_return(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);
        self.advance(); // Skip 'return'

        let value = if self.check(&Token::Semicolon) || self.check(&Token::Newline) || self.check(&Token::EOF) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        if self.check(&Token::Semicolon) {
            self.advance(); // Optional semicolon
        }

        Ok(Statement::Return { value, location })
    }

    fn parse_block(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);

        if !self.check(&Token::LeftBrace) {
            return Err(self.error("Expected '{' to start a block")
                .context_hint("if/while/function bodies need to be wrapped in { ... }"));
        }
        self.advance(); // Skip '{'

        let mut statements = Vec::new();

        while !self.check(&Token::RightBrace) && !self.check(&Token::EOF) {
            // Skip newlines
            while self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::RightBrace) || self.check(&Token::EOF) {
                break;
            }

            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }

        if !self.check(&Token::RightBrace) {
            return Err(self.error("Expected '}' to close this block")
                .context_hint("reached the end of the file while still inside a block -- check for a missing '}'"));
        }
        self.advance(); // Skip '}'

        Ok(Statement::Block { statements, location })
    }

    fn parse_if(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);
        self.advance(); // Skip 'if'

        while self.check(&Token::Newline) {
            self.advance();
        }

        // Condition is a plain expression; parentheses around it are optional
        // (e.g. both `if x > 0 {` and `if (x > 0) {` are accepted, since
        // parse_primary already handles a parenthesized sub-expression).
        let condition = self.parse_expression()?;

        while self.check(&Token::Newline) {
            self.advance();
        }

        let then_branch = self.parse_block()?;

        // Skip newlines before checking for 'else'
        let mut lookahead = self.position;
        while lookahead < self.tokens.len() && matches!(self.tokens[lookahead].token, Token::Newline) {
            lookahead += 1;
        }

        let else_branch = if lookahead < self.tokens.len() && matches!(self.tokens[lookahead].token, Token::Else) {
            self.position = lookahead;
            self.advance(); // Skip 'else'
            while self.check(&Token::Newline) {
                self.advance();
            }
            if self.check(&Token::If) {
                // 'else if ...' chains into another If statement
                Some(Box::new(self.parse_if()?))
            } else {
                Some(Box::new(self.parse_block()?))
            }
        } else {
            None
        };

        Ok(Statement::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
            location,
        })
    }

    fn parse_while(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);
        self.advance(); // Skip 'while'

        while self.check(&Token::Newline) {
            self.advance();
        }

        let condition = self.parse_expression()?;

        while self.check(&Token::Newline) {
            self.advance();
        }

        let body = self.parse_block()?;

        Ok(Statement::While {
            condition: Box::new(condition),
            body: Box::new(body),
            location,
        })
    }

    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression> {
        let mut left = self.parse_and()?;

        while self.check(&Token::Or) {
            let location = Location::new(self.current().line, self.current().column);
            self.advance();
            let right = self.parse_and()?;
            left = Expression::Binary {
                left: Box::new(left),
                operator: BinaryOperator::Or,
                right: Box::new(right),
                location,
            };
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression> {
        let mut left = self.parse_comparison()?;

        while self.check(&Token::And) {
            let location = Location::new(self.current().line, self.current().column);
            self.advance();
            let right = self.parse_comparison()?;
            left = Expression::Binary {
                left: Box::new(left),
                operator: BinaryOperator::And,
                right: Box::new(right),
                location,
            };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression> {
        let mut left = self.parse_term()?;

        loop {
            let location = Location::new(self.current().line, self.current().column);
            let operator = match &self.current().token {
                Token::EqualsEquals => { self.advance(); Some(BinaryOperator::Equal) }
                Token::NotEquals => { self.advance(); Some(BinaryOperator::NotEqual) }
                Token::LessThan => { self.advance(); Some(BinaryOperator::LessThan) }
                Token::GreaterThan => { self.advance(); Some(BinaryOperator::GreaterThan) }
                Token::LessEqual => { self.advance(); Some(BinaryOperator::LessEqual) }
                Token::GreaterEqual => { self.advance(); Some(BinaryOperator::GreaterEqual) }
                _ => None,
            };

            if let Some(op) = operator {
                let right = self.parse_term()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    operator: op,
                    right: Box::new(right),
                    location,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expression> {
        let mut left = self.parse_factor()?;

        loop {
            let location = Location::new(self.current().line, self.current().column);
            let operator = match &self.current().token {
                Token::Plus => { self.advance(); Some(BinaryOperator::Add) }
                Token::Minus => { self.advance(); Some(BinaryOperator::Subtract) }
                _ => None,
            };

            if let Some(op) = operator {
                let right = self.parse_factor()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    operator: op,
                    right: Box::new(right),
                    location,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expression> {
        let mut left = self.parse_unary()?;

        loop {
            let location = Location::new(self.current().line, self.current().column);
            let operator = match &self.current().token {
                Token::Star => { self.advance(); Some(BinaryOperator::Multiply) }
                Token::Slash => { self.advance(); Some(BinaryOperator::Divide) }
                _ => None,
            };

            if let Some(op) = operator {
                let right = self.parse_unary()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    operator: op,
                    right: Box::new(right),
                    location,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression> {
        let location = Location::new(self.current().line, self.current().column);

        let operator = match &self.current().token {
            Token::Not => { self.advance(); Some(UnaryOperator::Not) }
            Token::Minus => { self.advance(); Some(UnaryOperator::Negate) }
            _ => None,
        };

        if let Some(op) = operator {
            let operand = self.parse_unary()?;
            Ok(Expression::Unary {
                operator: op,
                operand: Box::new(operand),
                location,
            })
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expression> {
        let location = Location::new(self.current().line, self.current().column);

        // Clone the current token to avoid borrow issues
        let current_token = self.current().clone();

        match current_token.token {
            Token::Number(n) => {
                self.advance();
                Ok(Expression::Number {
                    value: n,
                    location,
                })
            }
            Token::String(s) => {
                self.advance();
                Ok(Expression::String {
                    value: s,
                    location,
                })
            }
            Token::True => {
                self.advance();
                Ok(Expression::Boolean {
                    value: true,
                    location,
                })
            }
            Token::False => {
                self.advance();
                Ok(Expression::Boolean {
                    value: false,
                    location,
                })
            }
            Token::Null => {
                self.advance();
                Ok(Expression::Null { location })
            }
            Token::Identifier(name) => {
                self.advance();

                // Check if it's a function call
                if self.check(&Token::LeftParen) {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.check(&Token::RightParen) && !self.check(&Token::EOF) {
                        // Skip newlines
                        while self.check(&Token::Newline) {
                            self.advance();
                        }

                        if self.check(&Token::RightParen) {
                            break;
                        }

                        let arg = self.parse_expression()?;
                        args.push(arg);

                        // Skip newlines before comma
                        while self.check(&Token::Newline) {
                            self.advance();
                        }

                        if !self.check(&Token::Comma) {
                            break;
                        }
                        self.advance(); // Skip comma
                    }

                    if !self.check(&Token::RightParen) {
                        return Err(self.error(format!("Expected ')' after arguments to '{}(...)'", name)));
                    }
                    self.advance(); // Skip ')'

                    Ok(Expression::Call {
                        function: Box::new(Expression::Identifier {
                            name: name.clone(),
                            location: location.clone(),
                        }),
                        arguments: args,
                        location,
                    })
                } else {
                    Ok(Expression::Identifier {
                        name,
                        location,
                    })
                }
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                if !self.check(&Token::RightParen) {
                    return Err(self.error("Expected ')' to close this parenthesized expression"));
                }
                self.advance();
                Ok(expr)
            }
            Token::Error => {
                Err(RozeError::lexer("Unrecognized character", current_token.line, current_token.column).into())
            }
            other => {
                Err(RozeError::parser(format!("Unexpected token: {}", other), current_token.line, current_token.column)
                    .with_length(token_len(&other))
                    .into())
            }
        }
    }
}

/// Small ergonomic helper so error sites can chain `.context_hint(...)`
/// straight onto `self.error(...)` without unwrapping and re-wrapping the
/// `anyhow::Error` by hand.
trait WithHint {
    fn context_hint(self, hint: impl Into<String>) -> Self;
}

impl WithHint for anyhow::Error {
    fn context_hint(self, hint: impl Into<String>) -> Self {
        match self.downcast::<RozeError>() {
            Ok(roze_err) => roze_err.with_hint(hint).into(),
            Err(other) => other,
        }
    }
}

pub fn parse(tokens: Vec<TokenWithLocation>) -> Result<Program> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}
