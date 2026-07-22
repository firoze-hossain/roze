// compiler/src/parser/ast.rs
#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let {
        name: String,
        value: Box<Expression>,
        location: Location,
    },
    Expression {
        expr: Box<Expression>,
        location: Location,
    },
    Return {
        value: Option<Box<Expression>>,
        location: Location,
    },
    Block {
        statements: Vec<Statement>,
        location: Location,
    },
    Function {
        name: String,
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        body: Box<Statement>,
        location: Location,
    },
    Import {
        path: String,
        location: Location,
    },
    If {
        condition: Box<Expression>,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
        location: Location,
    },
    While {
        condition: Box<Expression>,
        body: Box<Statement>,
        location: Location,
    },
    Assign {
        name: String,
        value: Box<Expression>,
        location: Location,
    },
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Number {
        value: String,
        location: Location,
    },
    String {
        value: String,
        location: Location,
    },
    Identifier {
        name: String,
        location: Location,
    },
    Boolean {
        value: bool,
        location: Location,
    },
    Null {
        location: Location,
    },
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
        location: Location,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
        location: Location,
    },
    Call {
        function: Box<Expression>,
        arguments: Vec<Expression>,
        location: Location,
    },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum UnaryOperator {
    Negate,
    Not,
}