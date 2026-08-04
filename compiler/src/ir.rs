// compiler/src/ir.rs
//
// A typed intermediate representation, sitting between the parser's
// (untyped) AST and codegen. Every expression node here carries its
// resolved `Type` (computed once, by the type checker), instead of each
// backend re-deriving types from the raw AST independently.
//
// Why this exists: before this module, `codegen::jvm` walked the raw
// AST and ran its *own*, separate type-inference pass (`infer_type`,
// plus a hand-rolled `Vec<HashMap<String, String>>` scope stack) to
// recover the same information the type checker had already computed
// and then thrown away. Two independent implementations of "what type
// is this expression" is exactly the kind of duplication that drifts --
// the same problem `compiler`'s own parser and the LSP's separate
// parser had (see ROADMAP.md's Phase 2 notes) before being unified.
// This IR is the fix, generalized: the type checker now *produces* a
// fully-annotated tree instead of just validating and discarding, and
// codegen consumes it directly.
//
// It also happens to be the right shape for adding a second backend
// later (native/LLVM, for the systems/embedded side of the project's
// goals -- see ROADMAP.md's "bigger picture" section): a typed IR that
// isn't tied to Java is what a second backend would need to consume
// anyway, so building one now (with only one backend) costs little and
// avoids a much larger untangling job once there are two.
use crate::parser::ast::{BinaryOperator, Location, UnaryOperator};
use crate::semantic::Type;

#[derive(Debug, Clone)]
pub struct TypedProgram {
    pub statements: Vec<TypedStatement>,
}

#[derive(Debug, Clone)]
pub struct TypedFunctionParam {
    pub name: String,
    pub type_: Type,
}

#[derive(Debug, Clone)]
pub enum TypedStatement {
    Let {
        name: String,
        value: TypedExpression,
        location: Location,
    },
    Expression {
        expr: TypedExpression,
        location: Location,
    },
    Return {
        value: Option<TypedExpression>,
        location: Location,
    },
    Block {
        statements: Vec<TypedStatement>,
        location: Location,
    },
    Function {
        name: String,
        params: Vec<TypedFunctionParam>,
        return_type: Type,
        body: Box<TypedStatement>,
        location: Location,
    },
    If {
        condition: TypedExpression,
        then_branch: Box<TypedStatement>,
        else_branch: Option<Box<TypedStatement>>,
        location: Location,
    },
    While {
        condition: TypedExpression,
        body: Box<TypedStatement>,
        location: Location,
    },
    Assign {
        name: String,
        value: TypedExpression,
        location: Location,
    },
    For {
        init: Box<TypedStatement>,
        condition: TypedExpression,
        update: Box<TypedStatement>,
        body: Box<TypedStatement>,
        location: Location,
    },
    /// A `class Name { field: type, ... }` declaration, fully resolved:
    /// each field's type is already known (declared explicitly, unlike
    /// `list`/`map` elements), which is what lets codegen (both
    /// backends) generate correct per-field handling directly from this
    /// node with no further lookup needed.
    ClassDecl {
        name: String,
        fields: Vec<(String, Type)>,
        location: Location,
    },
    /// `object.field = value;`
    FieldAssign {
        object: TypedExpression,
        field: String,
        /// The field's declared type, resolved once here by the type
        /// checker (which already has the class registry) so codegen
        /// doesn't need its own copy of that registry just to know
        /// whether the old/new value needs ARC retain/release.
        field_type: Type,
        value: TypedExpression,
        location: Location,
    },
    // Deliberately no `Import` variant: imports are resolved into real
    // functions before type-checking ever runs (see
    // imports::resolve_imports), so by the time this IR exists, no
    // Import statement is left anywhere in the tree.
}

/// An expression, annotated with its resolved type. `kind` is the
/// expression's shape (mirroring `parser::ast::Expression`); `type_` is
/// what the type checker determined it evaluates to -- e.g. an
/// `Identifier` here has already been resolved against the symbol table
/// at *this specific use site*, so codegen never needs to re-look-up a
/// variable's type via its own separate scope tracking.
#[derive(Debug, Clone)]
pub struct TypedExpression {
    pub kind: TypedExpressionKind,
    pub type_: Type,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub enum TypedExpressionKind {
    Number(String),
    String(String),
    Identifier(String),
    Boolean(bool),
    Null,
    Binary {
        left: Box<TypedExpression>,
        operator: BinaryOperator,
        right: Box<TypedExpression>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<TypedExpression>,
    },
    Call {
        // Roze has no first-class function values yet (no closures --
        // see ROADMAP.md), so a call's target is always a bare name,
        // never an arbitrary expression.
        function: String,
        arguments: Vec<TypedExpression>,
    },
    /// `new ClassName(arg1, arg2, ...)` -- arguments are positional, in
    /// the class's declared field order.
    New {
        class_name: String,
        arguments: Vec<TypedExpression>,
    },
    /// `object.field`
    FieldAccess {
        object: Box<TypedExpression>,
        field: String,
    },
}
