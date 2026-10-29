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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<String>,
}

impl TypeParam {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bounds: Vec::new(),
        }
    }

    pub fn with_bounds(name: impl Into<String>, bounds: Vec<String>) -> Self {
        Self {
            name: name.into(),
            bounds,
        }
    }
}

impl From<String> for TypeParam {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<&str> for TypeParam {
    fn from(name: &str) -> Self {
        Self::new(name)
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
        type_name: Option<String>,
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
        type_params: Vec<TypeParam>,
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        body: Box<Statement>,
        is_async: bool,
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
    For {
        init: Box<Statement>,
        condition: Box<Expression>,
        update: Box<Statement>,
        body: Box<Statement>,
        location: Location,
    },
    /// A `class Name { field: type, ... }` declaration -- fields only,
    /// deliberately no methods/inheritance/interfaces for this first
    /// increment (see ROADMAP.md). Reuses `FunctionParam` for fields
    /// since it's already exactly "a name plus an optional type name."
    ClassDecl {
        name: String,
        type_params: Vec<TypeParam>,
        fields: Vec<FunctionParam>,
        location: Location,
    },
    /// `object.field = value;` -- a distinct statement from `Assign`
    /// (which only ever targets a bare variable name) since the
    /// assignment target here is itself an expression (evaluated to
    /// find *which* object's field to write).
    FieldAssign {
        object: Box<Expression>,
        field: String,
        value: Box<Expression>,
        location: Location,
    },
    /// `enum Name { Variant1, Variant2(Type), ... }`
    EnumDecl {
        name: String,
        type_params: Vec<TypeParam>,
        variants: Vec<EnumVariant>,
        location: Location,
    },
    /// `match expr { Pattern => Body, ... }`
    Match {
        expr: Box<Expression>,
        arms: Vec<MatchArm>,
        location: Location,
    },
    /// `trait Name { func method(...) -> Type; ... }`
    TraitDecl {
        name: String,
        associated_types: Vec<String>,
        methods: Vec<TraitMethod>,
        location: Location,
    },
    /// `impl TraitName for ClassName { ... }` or `impl ClassName { ... }`
    ImplDecl {
        trait_name: Option<String>,
        class_name: String,
        associated_types: Vec<(String, String)>,
        methods: Vec<Statement>,
        location: Location,
    },
    /// `extern "C" func name(...) -> Type;`
    ExternFunction {
        abi: String,
        name: String,
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        location: Location,
    },
    /// `unsafe { ... }` block statement
    UnsafeBlock {
        body: Box<Statement>,
        location: Location,
    },
    /// `comptime { ... }` compile-time execution statement
    Comptime {
        statement: Box<Statement>,
        location: Location,
    },
}

#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<String>,
    pub location: Location,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnumVariant {
    pub name: String,
    pub payload_types: Vec<String>,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expression>>,
    pub body: Box<Statement>,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// `EnumName::Variant(p1, p2)` or `Variant(p1, p2)` or unit `EnumName::Variant` / `Variant`
    Variant {
        enum_name: Option<String>,
        variant: String,
        bindings: Vec<Pattern>,
        location: Location,
    },
    /// Struct / Class field destructuring: `Point { x, y }` or `Point { x: px, y: py }`
    Struct {
        class_name: String,
        fields: Vec<(String, Pattern)>,
        location: Location,
    },
    /// Tuple destructuring: `(p1, p2, ...)`
    Tuple {
        elements: Vec<Pattern>,
        location: Location,
    },
    /// Slice pattern: `[head, ..tail]` or `[first, second]` or `[]`
    Slice {
        prefix: Vec<Pattern>,
        slice_rest: Option<Box<Pattern>>,
        suffix: Vec<Pattern>,
        location: Location,
    },
    /// Range pattern: `1..=10` or `1..10`
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
        location: Location,
    },
    /// `_` wildcard
    Wildcard {
        location: Location,
    },
    /// Literal pattern, e.g. `42`, `"hello"`, `true`
    Literal {
        expr: Box<Expression>,
        location: Location,
    },
    /// Variable binding, e.g. `x`
    Variable {
        name: String,
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
    Char {
        value: char,
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
    /// `new ClassName<T1, ...>(arg1, arg2, ...)` -- arguments are positional, in
    /// the class's declared field order (see `Statement::ClassDecl`).
    New {
        class_name: String,
        type_args: Vec<String>,
        arguments: Vec<Expression>,
        location: Location,
    },
    /// Explicit generic function call: `fn_name<Type1, ...>(arg1, arg2, ...)`
    GenericCall {
        function: String,
        type_args: Vec<String>,
        arguments: Vec<Expression>,
        location: Location,
    },
    /// `object.field` -- reading a field. `object` is itself an
    /// expression (not just a bare name) so this composes with
    /// anything that evaluates to a class instance, e.g. a function
    /// call's return value.
    FieldAccess {
        object: Box<Expression>,
        field: String,
        location: Location,
    },
    /// `expr?` -- error propagation try operator. Unwraps a Result/Option or returns early from the current function.
    Try {
        expr: Box<Expression>,
        location: Location,
    },
    /// `EnumName::Variant(arg1, ...)` or `EnumName::Variant` (unit variant)
    EnumConstruct {
        enum_name: Option<String>,
        variant: String,
        arguments: Vec<Expression>,
        location: Location,
    },
    /// `|param1: Type, ...| -> ReturnType { ... }` or `|| { ... }`
    Closure {
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        body: Box<Statement>,
        location: Location,
    },
    /// `object.method(arg1, arg2, ...)`
    MethodCall {
        object: Box<Expression>,
        method: String,
        arguments: Vec<Expression>,
        location: Location,
    },
    /// `unsafe { expr }` or `unsafe expr`
    Unsafe {
        expr: Box<Expression>,
        location: Location,
    },
    /// `comptime <expr>` compile-time evaluated expression
    Comptime {
        expr: Box<Expression>,
        location: Location,
    },
    /// `comptime { <statements> }` compile-time evaluated block
    ComptimeBlock {
        block: Box<Statement>,
        location: Location,
    },
    /// `await <expr>` -- await completion of a Future / asynchronous task
    Await {
        expr: Box<Expression>,
        location: Location,
    },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
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