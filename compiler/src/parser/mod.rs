pub mod ast;

use crate::lexer::token::{Token, TokenWithLocation};
use ast::*;
use anyhow::{Result, anyhow};

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

    fn expect(&mut self, token_type: Token, message: &str) -> Result<TokenWithLocation> {
        if self.position >= self.tokens.len() {
            return Err(anyhow!("Unexpected end of file: {}", message));
        }

        let current_token = self.current().clone();
        // Compare by type, not by value
        if std::mem::discriminant(&current_token.token) == std::mem::discriminant(&token_type) {
            self.advance();
            Ok(current_token)
        } else {
            Err(anyhow!("{} at line {}: expected {:?}, found {:?}",
                message, current_token.line, token_type, current_token.token))
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
            Token::Func => {
                println!("DEBUG: Found Func token at line {}", self.current().line);
                self.parse_function()
            }
            Token::Let => self.parse_let(),
            Token::Return => self.parse_return(),
            Token::LeftBrace => self.parse_block(),
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
            _ => return Err(anyhow!("Invalid import path")),
        };

        Ok(Statement::Import { path, location })
    }

    fn parse_function(&mut self) -> Result<Statement> {
        let location = Location::new(self.current().line, self.current().column);

        // Skip 'func' token
        if self.check(&Token::Func) {
            self.advance();
            println!("DEBUG: Advanced past Func token");
        } else {
            return Err(anyhow!("Expected 'func' keyword at line {}", self.current().line));
        }

        // Skip any whitespace/newlines
        while self.check(&Token::Newline) {
            self.advance();
            println!("DEBUG: Skipped newline after func");
        }

        // Get function name - MUST be an identifier
        println!("DEBUG: Current token before expecting identifier: {:?}", self.current().token);

        // Use check for identifier type instead of expect
        if !self.check(&Token::Identifier(String::new())) {
            return Err(anyhow!("Expected function name at line {}, found {:?}",
                self.current().line, self.current().token));
        }

        let name_token = self.current().clone();
        let name = match &name_token.token {
            Token::Identifier(n) => n.clone(),
            _ => return Err(anyhow!("Invalid function name at line {}", name_token.line)),
        };
        self.advance();
        println!("DEBUG: Found function name: {}", name);

        // Skip newlines before parenthesis
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Expect opening parenthesis
        if !self.check(&Token::LeftParen) {
            return Err(anyhow!("Expected '(' after function name at line {}", self.current().line));
        }
        self.advance();
        println!("DEBUG: Found opening parenthesis");

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
                return Err(anyhow!("Expected parameter name at line {}", self.current().line));
            }

            let param_token = self.current().clone();
            let param_name = match &param_token.token {
                Token::Identifier(n) => n.clone(),
                _ => return Err(anyhow!("Invalid parameter name")),
            };
            self.advance();

            // Optional type annotation
            let param_type = if self.check(&Token::Colon) {
                self.advance();
                if !self.check(&Token::Identifier(String::new())) {
                    return Err(anyhow!("Expected type at line {}", self.current().line));
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
            return Err(anyhow!("Expected ')' after parameters at line {}", self.current().line));
        }
        self.advance();
        println!("DEBUG: Found closing parenthesis");

        // Skip newlines before body
        while self.check(&Token::Newline) {
            self.advance();
        }

        // Parse body (should be a block)
        let body = self.parse_statement()?;

        Ok(Statement::Function {
            name,
            params,
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
            return Err(anyhow!("Expected variable name at line {}", self.current().line));
        }

        let name_token = self.current().clone();
        let name = match name_token.token {
            Token::Identifier(n) => n,
            _ => return Err(anyhow!("Invalid variable name")),
        };
        self.advance();

        // Skip newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        if !self.check(&Token::Equals) {
            return Err(anyhow!("Expected '=' at line {}", self.current().line));
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
            return Err(anyhow!("Expected '{{' at line {}", self.current().line));
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
            return Err(anyhow!("Expected '}}' at line {}", self.current().line));
        }
        self.advance(); // Skip '}'

        Ok(Statement::Block { statements, location })
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
                        return Err(anyhow!("Expected ')' after arguments at line {}", self.current().line));
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
                    return Err(anyhow!("Expected ')' at line {}", self.current().line));
                }
                self.advance();
                Ok(expr)
            }
            _ => {
                Err(anyhow!("Unexpected token at line {}: {:?}", self.current().line, self.current().token))
            }
        }
    }
}

pub fn parse(tokens: Vec<TokenWithLocation>) -> Result<Program> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}