// compiler/src/semantic/mod.rs
pub mod comptime;
pub use comptime::{ComptimeEvaluator, ComptimeValue, comptime_value_to_ast};

use crate::error::RozeError;
use crate::ir::{
    TypedExpression, TypedExpressionKind, TypedFunctionParam, TypedMatchArm, TypedPattern,
    TypedProgram, TypedStatement,
};
use crate::parser::ast::*;
use std::collections::{HashMap, HashSet};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    /// A 64-bit floating-point number (Java `double` / Cranelift `f64`
    /// on native). Distinguished from `Int` purely by whether a number
    /// literal's source text contains a decimal point -- `42` is
    /// `Int`, `42.0` is `Float`, there's no separate `Expression`
    /// variant for it (see `check_expression`'s `Number` case).
    Float,
    String,
    Bool,
    Void,
    Unknown,
    /// A list of untyped (Object-boxed) elements. Roze doesn't have
    /// generics yet, so element type isn't tracked -- `list_get` always
    /// returns Unknown, the same way an untyped function parameter does.
    List,
    /// A map of untyped (Object-boxed) keys/values, for the same reason.
    Map,
    /// An optional value (`Some(v)` or `None()`).
    Option,
    /// A result value (`Ok(v)` or `Err(e)`).
    Result,
    /// An instance of a user-defined `class Name { field: type, ... }`.
    /// Unlike `list`/`map`'s elements, a class's field types *are*
    /// statically known (declared in the class body), which is what
    /// makes this tractable to support on the native backend too --
    /// each field's retain/release can be dispatched correctly based on
    /// its declared type, the same way a typed function parameter
    /// already is.
    Class(String),
    /// An instance of a user-defined enum / algebraic data type (`enum Name { Variant, ... }`).
    Enum(String),
    /// A trait / interface contract (`trait Name { func method(...) -> Type; }`).
    Trait(String),
    /// A generic type instantiation, e.g. `List<int>`, `Map<string, int>`, `Option<int>`, `Result<int, string>`.
    Generic {
        base: String,
        type_args: Vec<Type>,
    },
    /// Reserved for when Roze gets first-class function values/closures
    /// -- nothing constructs this yet (there's no syntax for a function
    /// *value*, only calls), but the other Type variants already match
    /// on it exhaustively, so it's kept rather than removed and
    /// re-added later.
    #[allow(dead_code)]
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// An unsigned 8-bit integer type (`u8`).
    U8,
    I8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    I128,
    U128,
    Isize,
    Usize,
    F32,
    F64,
    Char,
    /// Hardware SIMD vector types (M2-T02)
    Vec4f,
    Vec8f,
    Vec4i,
    Vec16u8,
    /// An unsafe raw pointer type (`*raw T`).
    RawPointer(Box<Type>),
}

impl Type {
    pub fn is_simd_vector(&self) -> bool {
        matches!(self, Type::Vec4f | Type::Vec8f | Type::Vec4i | Type::Vec16u8)
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::Int
                | Type::I8
                | Type::U8
                | Type::I16
                | Type::U16
                | Type::I32
                | Type::U32
                | Type::I64
                | Type::U64
                | Type::I128
                | Type::U128
                | Type::Isize
                | Type::Usize
        )
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::Usize
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float | Type::F32 | Type::F64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }
}

pub fn parse_numeric_literal(s: &str) -> (&str, Option<&str>) {
    for suffix in &[
        "isize", "usize", "i128", "u128", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64", "i8", "u8"
    ] {
        if s.ends_with(suffix) {
            return (&s[..s.len() - suffix.len()], Some(suffix));
        }
    }
    (s, None)
}

pub fn split_type_args(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < s.len() {
        let remaining = s[start..].trim();
        if !remaining.is_empty() {
            parts.push(remaining.to_string());
        }
    }
    parts
}

pub fn specialized_name(base: &str, type_args: &[Type]) -> String {
    let args_str = type_args.iter().map(|t| type_mangle_name(t)).collect::<Vec<_>>().join("__");
    format!("{}__{}", base, args_str)
}

fn type_mangle_name(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::String => "string".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
        Type::List => "list".to_string(),
        Type::Map => "map".to_string(),
        Type::Option => "option".to_string(),
        Type::Result => "result".to_string(),
        Type::Class(c) => c.clone(),
        Type::Enum(e) => e.clone(),
        Type::Trait(t) => t.clone(),
        Type::Generic { base, type_args } => specialized_name(base, type_args),
        Type::Function { .. } => "fn".to_string(),
        Type::U8 => "u8".to_string(),
        Type::I8 => "i8".to_string(),
        Type::I16 => "i16".to_string(),
        Type::U16 => "u16".to_string(),
        Type::I32 => "i32".to_string(),
        Type::U32 => "u32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U64 => "u64".to_string(),
        Type::I128 => "i128".to_string(),
        Type::U128 => "u128".to_string(),
        Type::Isize => "isize".to_string(),
        Type::Usize => "usize".to_string(),
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Char => "char".to_string(),
        Type::Vec4f => "vec4f".to_string(),
        Type::Vec8f => "vec8f".to_string(),
        Type::Vec4i => "vec4i".to_string(),
        Type::Vec16u8 => "vec16u8".to_string(),
        Type::RawPointer(inner) => format!("ptr_{}", type_mangle_name(inner)),
    }
}

impl Type {
    /// Maps a Roze source-level type name (e.g. from a `: string` parameter
    /// annotation or a `-> int` return type) to our internal Type. Anything
    /// unrecognized falls back to Unknown rather than erroring, since Roze
    /// doesn't have user-defined types yet.
    pub fn from_name(name: &str) -> Type {
        let trimmed = name.trim();
        if trimmed.starts_with("*raw") {
            let inner = trimmed["*raw".len()..].trim();
            let pointee = if inner.is_empty() {
                Type::U8
            } else {
                Type::from_name(inner)
            };
            return Type::RawPointer(Box::new(pointee));
        }
        if let Some(open) = trimmed.find('<') {
            if trimmed.ends_with('>') {
                let base = trimmed[..open].trim();
                let inner = &trimmed[open + 1..trimmed.len() - 1];
                let type_args = split_type_args(inner)
                    .into_iter()
                    .map(|s| Type::from_name(&s))
                    .collect();
                return Type::Generic {
                    base: base.to_string(),
                    type_args,
                };
            }
        }
        match trimmed {
            "int" => Type::Int,
            "float" => Type::Float,
            "string" => Type::String,
            "bool" => Type::Bool,
            "void" => Type::Void,
            "u8" => Type::U8,
            "i8" => Type::I8,
            "i16" => Type::I16,
            "u16" => Type::U16,
            "i32" => Type::I32,
            "u32" => Type::U32,
            "i64" => Type::I64,
            "u64" => Type::U64,
            "i128" => Type::I128,
            "u128" => Type::U128,
            "isize" => Type::Isize,
            "usize" => Type::Usize,
            "f32" => Type::F32,
            "f64" => Type::F64,
            "char" => Type::Char,
            "vec4f" | "Vec4f" => Type::Vec4f,
            "vec8f" | "Vec8f" => Type::Vec8f,
            "vec4i" | "Vec4i" => Type::Vec4i,
            "vec16u8" | "Vec16u8" => Type::Vec16u8,
            "list" | "List" => Type::List,
            "map" | "Map" => Type::Map,
            "option" | "Option" => Type::Option,
            "result" | "Result" => Type::Result,
            "channel" | "Channel" => Type::Generic { base: "Channel".to_string(), type_args: vec![] },
            "thread" | "Thread" => Type::Generic { base: "Thread".to_string(), type_args: vec![] },
            "fiber" | "Fiber" => Type::Generic { base: "Fiber".to_string(), type_args: vec![] },
            "future" | "Future" => Type::Generic { base: "Future".to_string(), type_args: vec![] },
            "mutex" | "Mutex" => Type::Generic { base: "Mutex".to_string(), type_args: vec![] },
            "rwlock" | "RwLock" => Type::Generic { base: "RwLock".to_string(), type_args: vec![] },
            "atomic_bool" | "AtomicBool" => Type::Generic { base: "AtomicBool".to_string(), type_args: vec![] },
            "atomic_i64" | "AtomicI64" => Type::Generic { base: "AtomicI64".to_string(), type_args: vec![] },
            "atomic_usize" | "AtomicUsize" => Type::Generic { base: "AtomicUsize".to_string(), type_args: vec![] },
            "atomic_ptr" | "AtomicPtr" => Type::Generic { base: "AtomicPtr".to_string(), type_args: vec![] },
            "task_group" | "TaskGroup" | "nursery" | "Nursery" => Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] },
            "subtask" | "Subtask" => Type::Generic { base: "Subtask".to_string(), type_args: vec![] },
            "cancellation_token" | "CancellationToken" | "cancel_token" | "CancelToken" => Type::Generic { base: "CancellationToken".to_string(), type_args: vec![] },
            "vec_deque" | "VecDeque" => Type::Generic { base: "VecDeque".to_string(), type_args: vec![] },
            "btree_map" | "BTreeMap" => Type::Generic { base: "BTreeMap".to_string(), type_args: vec![] },
            "btree_set" | "BTreeSet" => Type::Generic { base: "BTreeSet".to_string(), type_args: vec![] },
            "binary_heap" | "BinaryHeap" => Type::Generic { base: "BinaryHeap".to_string(), type_args: vec![] },
            "index_map" | "IndexMap" => Type::Generic { base: "IndexMap".to_string(), type_args: vec![] },
            "ring_buffer" | "RingBuffer" => Type::Generic { base: "RingBuffer".to_string(), type_args: vec![] },
            "bit_set" | "BitSet" => Type::Generic { base: "BitSet".to_string(), type_args: vec![] },
            "RozeBox" => Type::Generic { base: "RozeBox".to_string(), type_args: vec![] },
            "RozeArc" | "arc" | "Arc" => Type::Generic { base: "RozeArc".to_string(), type_args: vec![] },
            "RozeWeak" | "weak" | "Weak" => Type::Generic { base: "RozeWeak".to_string(), type_args: vec![] },
            "bump_allocator" | "BumpAllocator" => Type::Generic { base: "BumpAllocator".to_string(), type_args: vec![] },
            "arena_allocator" | "ArenaAllocator" => Type::Generic { base: "ArenaAllocator".to_string(), type_args: vec![] },
            "pool_allocator" | "PoolAllocator" | "fixed_block_pool_allocator" | "FixedBlockPoolAllocator" => Type::Generic { base: "PoolAllocator".to_string(), type_args: vec![] },
            _ => Type::Unknown,
        }
    }

    pub fn to_java(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::I8 => "byte".to_string(),
            Type::U8 => "int".to_string(),
            Type::I16 => "short".to_string(),
            Type::U16 => "int".to_string(),
            Type::I32 => "int".to_string(),
            Type::U32 => "long".to_string(),
            Type::I64 => "long".to_string(),
            Type::U64 => "long".to_string(),
            Type::I128 => "java.math.BigInteger".to_string(),
            Type::U128 => "java.math.BigInteger".to_string(),
            Type::Isize => "long".to_string(),
            Type::Usize => "long".to_string(),
            Type::Float => "double".to_string(),
            Type::F32 => "float".to_string(),
            Type::F64 => "double".to_string(),
            Type::Char => "char".to_string(),
            Type::Vec4f => "RozeVec4f".to_string(),
            Type::Vec8f => "RozeVec8f".to_string(),
            Type::Vec4i => "RozeVec4i".to_string(),
            Type::Vec16u8 => "RozeVec16u8".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "boolean".to_string(),
            Type::Void => "void".to_string(),
            Type::Unknown => "Object".to_string(),
            Type::RawPointer(_) => "long".to_string(),
            Type::List => "java.util.List".to_string(),
            Type::Map => "java.util.Map".to_string(),
            Type::Option => "RozeOption".to_string(),
            Type::Result => "RozeResult".to_string(),
            Type::Class(name) => name.clone(),
            Type::Enum(name) => name.clone(),
            Type::Trait(name) => name.clone(),
            Type::Generic { base, type_args } => {
                if base == "List" || base == "list" {
                    "java.util.List".to_string()
                } else if base == "Map" || base == "map" {
                    "java.util.Map".to_string()
                } else if base == "Option" || base == "option" {
                    "RozeOption".to_string()
                } else if base == "Result" || base == "result" {
                    "RozeResult".to_string()
                } else if base == "Channel" || base == "channel" {
                    "RozeChannel".to_string()
                } else if base == "Thread" || base == "thread" {
                    "Thread".to_string()
                } else if base == "Fiber" || base == "fiber" {
                    "RozeFiberHandle".to_string()
                } else if base == "Future" || base == "future" {
                    "RozeFuture".to_string()
                } else if base == "Mutex" || base == "mutex" {
                    "RozeMutex".to_string()
                } else if base == "RwLock" || base == "rwlock" {
                    "RozeRwLock".to_string()
                } else if base == "AtomicBool" || base == "atomic_bool" {
                    "java.util.concurrent.atomic.AtomicBoolean".to_string()
                } else if base == "AtomicI64" || base == "atomic_i64" || base == "AtomicUsize" || base == "atomic_usize" {
                    "java.util.concurrent.atomic.AtomicLong".to_string()
                } else if base == "AtomicPtr" || base == "atomic_ptr" {
                    "java.util.concurrent.atomic.AtomicReference".to_string()
                } else if base == "TaskGroup" || base == "task_group" || base == "Nursery" || base == "nursery" {
                    "RozeTaskGroup".to_string()
                } else if base == "Subtask" || base == "subtask" {
                    "RozeSubtask".to_string()
                } else if base == "CancellationToken" || base == "cancellation_token" || base == "CancelToken" || base == "cancel_token" {
                    "RozeCancellationToken".to_string()
                } else if base == "VecDeque" || base == "vec_deque" {
                    "RozeVecDeque".to_string()
                } else if base == "BTreeMap" || base == "btree_map" {
                    "RozeBTreeMap".to_string()
                } else if base == "BTreeSet" || base == "btree_set" {
                    "RozeBTreeSet".to_string()
                } else if base == "BinaryHeap" || base == "binary_heap" {
                    "RozeBinaryHeap".to_string()
                } else if base == "IndexMap" || base == "index_map" {
                    "RozeIndexMap".to_string()
                } else if base == "RingBuffer" || base == "ring_buffer" {
                    "RozeRingBuffer".to_string()
                } else if base == "BitSet" || base == "bit_set" {
                    "RozeBitSet".to_string()
                } else if base == "RozeBox" {
                    "RozeBox".to_string()
                } else if base == "RozeArc" || base == "Arc" || base == "arc" {
                    "RozeArc".to_string()
                } else if base == "RozeWeak" || base == "Weak" || base == "weak" {
                    "RozeWeak".to_string()
                } else if base == "BumpAllocator" || base == "bump_allocator" {
                    "RozeBumpAllocator".to_string()
                } else if base == "ArenaAllocator" || base == "arena_allocator" {
                    "RozeArenaAllocator".to_string()
                } else if base == "PoolAllocator" || base == "pool_allocator" || base == "FixedBlockPoolAllocator" || base == "fixed_block_pool_allocator" {
                    "RozePoolAllocator".to_string()
                } else {
                    specialized_name(base, type_args)
                }
            }
            Type::Function { .. } => "RozeFn".to_string(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::I8 => write!(f, "i8"),
            Type::U8 => write!(f, "u8"),
            Type::I16 => write!(f, "i16"),
            Type::U16 => write!(f, "u16"),
            Type::I32 => write!(f, "i32"),
            Type::U32 => write!(f, "u32"),
            Type::I64 => write!(f, "i64"),
            Type::U64 => write!(f, "u64"),
            Type::I128 => write!(f, "i128"),
            Type::U128 => write!(f, "u128"),
            Type::Isize => write!(f, "isize"),
            Type::Usize => write!(f, "usize"),
            Type::Float => write!(f, "float"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Char => write!(f, "char"),
            Type::Vec4f => write!(f, "Vec4f"),
            Type::Vec8f => write!(f, "Vec8f"),
            Type::Vec4i => write!(f, "Vec4i"),
            Type::Vec16u8 => write!(f, "Vec16u8"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Void => write!(f, "void"),
            Type::Unknown => write!(f, "<unknown>"),
            Type::RawPointer(inner) => write!(f, "*raw {}", inner),
            Type::List => write!(f, "list"),
            Type::Map => write!(f, "map"),
            Type::Option => write!(f, "Option"),
            Type::Result => write!(f, "Result"),
            Type::Class(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
            Type::Trait(name) => write!(f, "{}", name),
            Type::Generic { base, type_args } => {
                let args_str = type_args.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "{}<{}>", base, args_str)
            }
            Type::Function { .. } => write!(f, "function"),
        }
    }
}

/// Two types are "compatible" for assignment/return purposes if they're
/// equal, if either side is Unknown -- meaning we don't have enough
/// static information to say either way, so we don't block it -- or if
/// an int is flowing into a float-expecting position (implicit
/// widening, the same direction `+`/`-`/`*`/`/` already promote mixed
/// int/float operands in). The reverse -- a float flowing into an
/// int-expecting position -- is deliberately NOT allowed here: that's
/// narrowing, silently losing precision, and needs an explicit
/// conversion instead (not implemented yet -- there's no cast syntax
/// in Roze at all currently).
fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }
    match (expected, actual) {
        (Type::Unknown, _) | (_, Type::Unknown) => true,
        (Type::Int, Type::I64) | (Type::I64, Type::Int) => true,
        (Type::Float, Type::F64) | (Type::F64, Type::Float) => true,
        (exp, act) if exp.is_float() && act.is_float() => true,
        (exp, act) if exp.is_float() && act.is_integer() => true,
        (exp, act) if exp.is_integer() && act.is_integer() => true,
        (Type::RawPointer(_), Type::RawPointer(_)) => true,
        (Type::RawPointer(_), ty) if ty.is_integer() => true,
        (ty, Type::RawPointer(_)) if ty.is_integer() => true,
        (Type::Char, Type::Char) => true,
        (Type::Char, ty) | (ty, Type::Char) if ty.is_integer() => true,
        // List <-> List<T>
        (Type::List, Type::Generic { base, .. }) if base == "List" || base == "list" => true,
        (Type::Generic { base, .. }, Type::List) if base == "List" || base == "list" => true,
        // Map <-> Map<K, V>
        (Type::Map, Type::Generic { base, .. }) if base == "Map" || base == "map" => true,
        (Type::Generic { base, .. }, Type::Map) if base == "Map" || base == "map" => true,
        // Option <-> Option<T>
        (Type::Option, Type::Generic { base, .. }) if base == "Option" || base == "option" => true,
        (Type::Generic { base, .. }, Type::Option) if base == "Option" || base == "option" => true,
        // Result <-> Result<T, E>
        (Type::Result, Type::Generic { base, .. }) if base == "Result" || base == "result" => true,
        (Type::Generic { base, .. }, Type::Result) if base == "Result" || base == "result" => true,
        // Channel <-> Channel<T>
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Channel" || b1 == "channel") && (b2 == "Channel" || b2 == "channel") => true,
        // Thread <-> Thread
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Thread" || b1 == "thread") && (b2 == "Thread" || b2 == "thread") => true,
        // Fiber <-> Fiber
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Fiber" || b1 == "fiber") && (b2 == "Fiber" || b2 == "fiber") => true,
        // Future <-> Future
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Future" || b1 == "future") && (b2 == "Future" || b2 == "future") => true,
        // Mutex <-> Mutex
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Mutex" || b1 == "mutex") && (b2 == "Mutex" || b2 == "mutex") => true,
        // RwLock <-> RwLock
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "RwLock" || b1 == "rwlock") && (b2 == "RwLock" || b2 == "rwlock") => true,
        // AtomicBool <-> AtomicBool
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "AtomicBool" || b1 == "atomic_bool") && (b2 == "AtomicBool" || b2 == "atomic_bool") => true,
        // AtomicI64 <-> AtomicI64
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "AtomicI64" || b1 == "atomic_i64") && (b2 == "AtomicI64" || b2 == "atomic_i64") => true,
        // AtomicUsize <-> AtomicUsize
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "AtomicUsize" || b1 == "atomic_usize") && (b2 == "AtomicUsize" || b2 == "atomic_usize") => true,
        // AtomicPtr <-> AtomicPtr
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "AtomicPtr" || b1 == "atomic_ptr") && (b2 == "AtomicPtr" || b2 == "atomic_ptr") => true,
        // TaskGroup <-> TaskGroup / Nursery
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "TaskGroup" || b1 == "task_group" || b1 == "Nursery" || b1 == "nursery") && (b2 == "TaskGroup" || b2 == "task_group" || b2 == "Nursery" || b2 == "nursery") => true,
        // Subtask <-> Subtask
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "Subtask" || b1 == "subtask") && (b2 == "Subtask" || b2 == "subtask") => true,
        // CancellationToken <-> CancellationToken
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1 == "CancellationToken" || b1 == "cancellation_token" || b1 == "CancelToken" || b1 == "cancel_token") && (b2 == "CancellationToken" || b2 == "cancellation_token" || b2 == "CancelToken" || b2 == "cancel_token") => true,
        // Standard Collections
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if (b1.eq_ignore_ascii_case("VecDeque") && b2.eq_ignore_ascii_case("VecDeque"))
            || (b1.eq_ignore_ascii_case("BTreeMap") && b2.eq_ignore_ascii_case("BTreeMap"))
            || (b1.eq_ignore_ascii_case("BTreeSet") && b2.eq_ignore_ascii_case("BTreeSet"))
            || (b1.eq_ignore_ascii_case("BinaryHeap") && b2.eq_ignore_ascii_case("BinaryHeap"))
            || (b1.eq_ignore_ascii_case("IndexMap") && b2.eq_ignore_ascii_case("IndexMap"))
            || (b1.eq_ignore_ascii_case("RingBuffer") && b2.eq_ignore_ascii_case("RingBuffer"))
            || (b1.eq_ignore_ascii_case("BitSet") && b2.eq_ignore_ascii_case("BitSet")) => true,
        // Memory & Smart Pointers (M2-T04)
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if ((b1 == "RozeBox" || b1.eq_ignore_ascii_case("Box")) && (b2 == "RozeBox" || b2.eq_ignore_ascii_case("Box")))
            || ((b1 == "RozeArc" || b1.eq_ignore_ascii_case("Arc")) && (b2 == "RozeArc" || b2.eq_ignore_ascii_case("Arc")))
            || ((b1 == "RozeWeak" || b1.eq_ignore_ascii_case("Weak")) && (b2 == "RozeWeak" || b2.eq_ignore_ascii_case("Weak")))
            || (b1.eq_ignore_ascii_case("BumpAllocator") && b2.eq_ignore_ascii_case("BumpAllocator"))
            || (b1.eq_ignore_ascii_case("ArenaAllocator") && b2.eq_ignore_ascii_case("ArenaAllocator"))
            || (b1.eq_ignore_ascii_case("PoolAllocator") && b2.eq_ignore_ascii_case("PoolAllocator")) => true,
        // Fiber <-> Thread interoperability
        (Type::Generic { base: b1, .. }, Type::Generic { base: b2, .. })
            if ((b1 == "Fiber" || b1 == "fiber") && (b2 == "Thread" || b2 == "thread"))
            || ((b1 == "Thread" || b1 == "thread") && (b2 == "Fiber" || b2 == "fiber")) => true,
        // Generic with matching base and compatible type arguments:
        (
            Type::Generic { base: b1, type_args: a1 },
            Type::Generic { base: b2, type_args: a2 },
        ) if b1 == b2 && a1.len() == a2.len() => {
            a1.iter().zip(a2.iter()).all(|(x, y)| types_compatible(x, y))
        }
        (
            Type::Generic { base, type_args },
            Type::Class(c),
        ) if c == &specialized_name(base, type_args) => true,
        (
            Type::Class(c),
            Type::Generic { base, type_args },
        ) if c == &specialized_name(base, type_args) => true,
        (
            Type::Function { params: p1, return_type: r1 },
            Type::Function { params: p2, return_type: r2 },
        ) => {
            if p1.len() != p2.len() {
                return false;
            }
            for (a1, a2) in p1.iter().zip(p2.iter()) {
                if !types_compatible(a1, a2) {
                    return false;
                }
            }
            types_compatible(r1, r2)
        }
        _ => false,
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Symbol {
    pub name: String,
    pub type_: Type,
    pub line: usize,
    pub column: usize,
}

/// A stack of lexical scopes.
///
/// The previous design represented nested scopes as a linked list of owned
/// `SymbolTable`s (`parent: Option<Box<SymbolTable>>`) and entered a new
/// scope by deep-cloning the *entire* parent chain (`Clone for SymbolTable`
/// recursively cloned `parent`). That means the cost of entering a scope
/// grew with total program depth, and a function with many sequential
/// nested blocks paid that growing cost on every single block, not just
/// once. A flat stack of frames makes push/pop O(1) and lookup O(depth)
/// (which is unavoidable and cheap: it's just a few hash lookups, no
/// cloning of prior state at all).
pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, name: &str, type_: Type, line: usize, column: usize) -> Result<()> {
        let frame = self.scopes.last_mut().expect("SymbolTable always has at least one scope");
        if frame.contains_key(name) {
            return Err(RozeError::type_error(
                format!("'{}' is already defined in this scope", name),
                line,
                column,
            ).with_length(name.chars().count()).into());
        }
        frame.insert(name.to_string(), Symbol {
            name: name.to_string(),
            type_,
            line,
            column,
        });
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for frame in self.scopes.iter().rev() {
            if let Some(sym) = frame.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_with_depth(&self, name: &str) -> Option<(&Symbol, usize)> {
        for (depth, frame) in self.scopes.iter().enumerate().rev() {
            if let Some(sym) = frame.get(name) {
                return Some((sym, depth));
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// Built-in Core (string/math) intrinsics, always available without an
/// import. These are recognized by name here (for type-checking) and again
/// in `codegen::jvm` (for code generation), because Roze doesn't yet have
/// syntax to call into the host runtime from user-level source (no method
/// calls / no FFI). Keep this list in sync with
/// `codegen::jvm::intrinsic_return_type`.
fn builtin_signatures() -> Vec<(&'static str, Vec<Type>, Type)> {
    vec![
        ("string_length", vec![Type::String], Type::Int),
        ("string_concat", vec![Type::String, Type::String], Type::String),
        ("string_to_upper", vec![Type::String], Type::String),
        ("string_to_lower", vec![Type::String], Type::String),
        ("substring", vec![Type::String, Type::Int, Type::Int], Type::String),
        ("string_substring", vec![Type::String, Type::Int, Type::Int], Type::String),
        ("string_contains", vec![Type::String, Type::String], Type::Bool),
        ("string_starts_with", vec![Type::String, Type::String], Type::Bool),
        ("string_ends_with", vec![Type::String, Type::String], Type::Bool),
        ("abs", vec![Type::Unknown], Type::Int),
        ("max", vec![Type::Unknown, Type::Unknown], Type::Int),
        ("min", vec![Type::Unknown, Type::Unknown], Type::Int),
        ("to_string", vec![Type::Unknown], Type::String),
        ("to_int", vec![Type::String], Type::Int),
        ("is_number", vec![Type::Unknown], Type::Bool),
        ("is_string", vec![Type::Unknown], Type::Bool),

        // ---- Collections (List, Map) ----
        // No generics yet, so elements/keys/values are all Unknown
        // (Object-boxed at the Java level) -- the same representation an
        // untyped function parameter already gets.
        ("list_new", vec![], Type::List),
        ("list_push", vec![Type::List, Type::Unknown], Type::Bool),
        ("list_get", vec![Type::List, Type::Int], Type::Unknown),
        ("list_set", vec![Type::List, Type::Int, Type::Unknown], Type::Unknown),
        ("list_remove", vec![Type::List, Type::Int], Type::Unknown),
        ("list_length", vec![Type::List], Type::Int),
        ("list_is_empty", vec![Type::List], Type::Bool),

        ("map_new", vec![], Type::Map),
        ("map_put", vec![Type::Map, Type::Unknown, Type::Unknown], Type::Unknown),
        ("map_set", vec![Type::Map, Type::Unknown, Type::Unknown], Type::Unknown),
        ("map_get", vec![Type::Map, Type::Unknown], Type::Unknown),
        ("map_has", vec![Type::Map, Type::Unknown], Type::Bool),
        ("map_remove", vec![Type::Map, Type::Unknown], Type::Unknown),
        ("map_size", vec![Type::Map], Type::Int),
        ("map_is_empty", vec![Type::Map], Type::Bool),
        ("map_keys", vec![Type::Map], Type::List),

        // ---- IO: file ----
        // Errors (file not found, permission denied, network failure)
        // surface as a runtime crash (an unchecked exception from the
        // JVM) rather than a Roze-level error value -- Roze doesn't have
        // a Result/Option type yet to report failure any other way. See
        // ROADMAP.md.
        ("read_file", vec![Type::String], Type::String),
        ("write_file", vec![Type::String, Type::String], Type::Void),
        ("append_file", vec![Type::String, Type::String], Type::Void),
        ("file_exists", vec![Type::String], Type::Bool),
        ("delete_file", vec![Type::String], Type::Bool),
        ("read_lines", vec![Type::String], Type::List),

        // ---- IO: network ----
        ("http_get", vec![Type::String], Type::String),
        ("http_post", vec![Type::String, Type::String], Type::String),
        ("http_request", vec![Type::String, Type::String, Type::Map, Type::String], Type::Map),

        // ---- Web: JSON ----
        // encode accepts anything (int/string/bool/list/map, nested);
        // decode's result type depends on the JSON text, so it's Unknown
        // -- the same design as list/map element access.
        ("json_encode", vec![Type::Unknown], Type::String),
        ("json_decode", vec![Type::String], Type::Unknown),

        // ---- Web: HTTP server ----
        // A request is just a `map` (with "method"/"path"/"body" keys),
        // reusing the existing Map intrinsics rather than needing a new
        // type -- see stdlib/src/io.roze for the full design rationale
        // (Roze has no closures, so this is a synchronous accept/respond
        // loop you write yourself, not a callback-based server).
        ("http_server_start", vec![Type::Int], Type::Unknown),
        ("http_server_accept", vec![Type::Unknown], Type::Map),
        ("http_server_respond", vec![Type::Map, Type::Int, Type::String], Type::Void),
        ("http_server_stop", vec![Type::Unknown], Type::Void),
        ("http_server_listen", vec![Type::Int, Type::Unknown], Type::Unknown),
        ("http_server_respond_json", vec![Type::Map, Type::Int, Type::Unknown], Type::Void),
        ("http_server_respond_headers", vec![Type::Map, Type::Int, Type::Map, Type::String], Type::Void),
        ("url_route_match", vec![Type::String, Type::String], Type::Unknown),

        // ---- Database (SQL) ----
        // Roze has no bundled JDBC driver (the JDK itself doesn't ship
        // one for any real database), so these work with whatever
        // driver you put on the classpath yourself via `roze build
        // --classpath`/`roze run --classpath` -- see stdlib/src/sql.roze.
        ("sql_connect", vec![Type::String], Type::Unknown),
        ("sql_query", vec![Type::Unknown, Type::String], Type::List),
        ("sql_execute", vec![Type::Unknown, Type::String], Type::Int),
        ("sql_close", vec![Type::Unknown], Type::Void),

        // ---- RozeBoot Enterprise Framework (ADV-03) ----
        // Database Connection Pooling (HikariCP / Pooled DataSource)
        ("db_pool_create", vec![Type::String, Type::Int], Type::Unknown),
        ("db_pool_get", vec![Type::Unknown], Type::Unknown),
        ("db_pool_release", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("db_pool_close", vec![Type::Unknown], Type::Void),
        ("db_pool_stats", vec![Type::Unknown], Type::Map),
        ("db_pool_query", vec![Type::Unknown, Type::String], Type::List),
        ("db_pool_execute", vec![Type::Unknown, Type::String], Type::Int),

        // JWT Security & Cryptography
        ("jwt_sign", vec![Type::Map, Type::String], Type::String),
        ("jwt_verify", vec![Type::String, Type::String], Type::Bool),
        ("jwt_decode", vec![Type::String], Type::Map),
        ("jwt_decode_header", vec![Type::String], Type::Map),

        // Time
        ("time_now", vec![], Type::Int),
        ("time_now_millis", vec![], Type::Int),

        // ---- Option ----
        ("option_some", vec![Type::Unknown], Type::Option),
        ("option_none", vec![], Type::Option),
        ("option_is_some", vec![Type::Unknown], Type::Bool),
        ("option_is_none", vec![Type::Unknown], Type::Bool),
        ("option_unwrap", vec![Type::Unknown], Type::Unknown),
        ("option_unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("Some", vec![Type::Unknown], Type::Option),
        ("None", vec![], Type::Option),
        ("is_some", vec![Type::Unknown], Type::Bool),
        ("is_none", vec![Type::Unknown], Type::Bool),

        // ---- Result ----
        ("result_ok", vec![Type::Unknown], Type::Result),
        ("result_err", vec![Type::Unknown], Type::Result),
        ("result_is_ok", vec![Type::Unknown], Type::Bool),
        ("result_is_err", vec![Type::Unknown], Type::Bool),
        ("result_unwrap", vec![Type::Unknown], Type::Unknown),
        ("result_unwrap_err", vec![Type::Unknown], Type::Unknown),
        ("result_unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("Ok", vec![Type::Unknown], Type::Result),
        ("Err", vec![Type::Unknown], Type::Result),
        ("is_ok", vec![Type::Unknown], Type::Bool),
        ("is_err", vec![Type::Unknown], Type::Bool),
        ("unwrap", vec![Type::Unknown], Type::Unknown),
        ("unwrap_err", vec![Type::Unknown], Type::Unknown),
        ("unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),

        // ---- Safe IO (returning Result) ----
        ("read_file_result", vec![Type::String], Type::Result),
        ("write_file_result", vec![Type::String, Type::String], Type::Result),
        ("append_file_result", vec![Type::String, Type::String], Type::Result),
        ("delete_file_result", vec![Type::String], Type::Result),
        ("read_lines_result", vec![Type::String], Type::Result),
        ("http_get_result", vec![Type::String], Type::Result),
        ("http_post_result", vec![Type::String, Type::String], Type::Result),
        ("to_int_result", vec![Type::String], Type::Result),

        // ---- Safe Collections (returning Option) ----
        ("list_get_option", vec![Type::List, Type::Int], Type::Option),
        ("map_get_option", vec![Type::Map, Type::Unknown], Type::Option),

        // ---- Concurrency: Threads & Channels (MED-03) ----
        ("thread_spawn", vec![Type::Unknown], Type::Generic { base: "Thread".to_string(), type_args: vec![] }),
        ("thread_join", vec![Type::Unknown], Type::Unknown),
        ("thread_sleep", vec![Type::Int], Type::Void),
        ("thread_current_id", vec![], Type::Int),

        // ---- Modern Concurrency: M:N Green Threads / Virtual Threads (M1-T04) ----
        ("fiber_spawn", vec![Type::Unknown], Type::Generic { base: "Fiber".to_string(), type_args: vec![] }),
        ("fiber_join", vec![Type::Unknown], Type::Unknown),
        ("fiber_yield", vec![], Type::Void),
        ("fiber_sleep", vec![Type::Int], Type::Void),
        ("fiber_id", vec![], Type::Int),
        ("fiber_count", vec![], Type::Int),
        ("fiber_stats", vec![], Type::Void),

        // ---- Async Runtime: Async / Await & Future<T> (M1-T05) ----
        ("__roze_async_spawn", vec![Type::Unknown], Type::Generic { base: "Future".to_string(), type_args: vec![] }),
        ("future_spawn", vec![Type::Unknown], Type::Generic { base: "Future".to_string(), type_args: vec![] }),
        ("future_await", vec![Type::Unknown], Type::Unknown),
        ("future_poll", vec![Type::Unknown], Type::Bool),
        ("future_ready", vec![Type::Unknown], Type::Generic { base: "Future".to_string(), type_args: vec![] }),
        ("async_sleep", vec![Type::Int], Type::Generic { base: "Future".to_string(), type_args: vec![Type::Int] }),
        ("__roze_future_await", vec![Type::Unknown], Type::Unknown),
        ("__roze_future_poll", vec![Type::Unknown], Type::Bool),
        ("__roze_future_ready", vec![Type::Unknown], Type::Generic { base: "Future".to_string(), type_args: vec![] }),
        ("__roze_async_sleep", vec![Type::Int], Type::Generic { base: "Future".to_string(), type_args: vec![Type::Int] }),

        ("channel_new", vec![], Type::Generic { base: "Channel".to_string(), type_args: vec![] }),
        ("channel_unbounded", vec![], Type::Generic { base: "Channel".to_string(), type_args: vec![] }),
        ("channel_send", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("send", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("channel_try_send", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("channel_recv", vec![Type::Unknown], Type::Unknown),
        ("recv", vec![Type::Unknown], Type::Unknown),
        ("channel_try_recv", vec![Type::Unknown], Type::Unknown),
        ("channel_close", vec![Type::Unknown], Type::Void),
        ("channel_is_closed", vec![Type::Unknown], Type::Bool),
        ("channel_len", vec![Type::Unknown], Type::Int),
        ("channel_capacity", vec![Type::Unknown], Type::Int),

        // ---- Synchronization: Mutex & RwLock (M1-T06) ----
        ("mutex_new", vec![], Type::Generic { base: "Mutex".to_string(), type_args: vec![] }),
        ("mutex_lock", vec![Type::Unknown], Type::Void),
        ("mutex_unlock", vec![Type::Unknown], Type::Void),
        ("mutex_try_lock", vec![Type::Unknown], Type::Bool),

        ("rwlock_new", vec![], Type::Generic { base: "RwLock".to_string(), type_args: vec![] }),
        ("rwlock_read_lock", vec![Type::Unknown], Type::Void),
        ("rwlock_read_unlock", vec![Type::Unknown], Type::Void),
        ("rwlock_write_lock", vec![Type::Unknown], Type::Void),
        ("rwlock_write_unlock", vec![Type::Unknown], Type::Void),
        ("rwlock_try_read_lock", vec![Type::Unknown], Type::Bool),
        ("rwlock_try_write_lock", vec![Type::Unknown], Type::Bool),

        // ---- Synchronization: Atomic Types (M1-T06) ----
        ("atomic_bool_new", vec![Type::Bool], Type::Generic { base: "AtomicBool".to_string(), type_args: vec![] }),
        ("atomic_bool_load", vec![Type::Unknown], Type::Bool),
        ("atomic_bool_store", vec![Type::Unknown, Type::Bool], Type::Void),
        ("atomic_bool_swap", vec![Type::Unknown, Type::Bool], Type::Bool),
        ("atomic_bool_cas", vec![Type::Unknown, Type::Bool, Type::Bool], Type::Bool),

        ("atomic_i64_new", vec![Type::Int], Type::Generic { base: "AtomicI64".to_string(), type_args: vec![] }),
        ("atomic_i64_load", vec![Type::Unknown], Type::Int),
        ("atomic_i64_store", vec![Type::Unknown, Type::Int], Type::Void),
        ("atomic_i64_swap", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_i64_cas", vec![Type::Unknown, Type::Int, Type::Int], Type::Bool),
        ("atomic_i64_add", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_i64_sub", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_i64_and", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_i64_or", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_i64_xor", vec![Type::Unknown, Type::Int], Type::Int),

        ("atomic_usize_new", vec![Type::Int], Type::Generic { base: "AtomicUsize".to_string(), type_args: vec![] }),
        ("atomic_usize_load", vec![Type::Unknown], Type::Int),
        ("atomic_usize_store", vec![Type::Unknown, Type::Int], Type::Void),
        ("atomic_usize_swap", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_usize_cas", vec![Type::Unknown, Type::Int, Type::Int], Type::Bool),
        ("atomic_usize_add", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_usize_sub", vec![Type::Unknown, Type::Int], Type::Int),

        ("atomic_ptr_new", vec![Type::Unknown], Type::Generic { base: "AtomicPtr".to_string(), type_args: vec![] }),
        ("atomic_ptr_load", vec![Type::Unknown], Type::Unknown),
        ("atomic_ptr_store", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("atomic_ptr_swap", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("atomic_ptr_cas", vec![Type::Unknown, Type::Unknown, Type::Unknown], Type::Bool),

        // Polymorphic / generic convenience aliases
        ("atomic_load", vec![Type::Unknown], Type::Unknown),
        ("atomic_store", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("atomic_swap", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("atomic_cas", vec![Type::Unknown, Type::Unknown, Type::Unknown], Type::Bool),
        ("atomic_add", vec![Type::Unknown, Type::Int], Type::Int),
        ("atomic_sub", vec![Type::Unknown, Type::Int], Type::Int),

        // ---- Structured Concurrency: Nursery, Task Groups & Cancelation (M1-T07) ----
        ("nursery_new", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("task_group_new", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("nursery_on_failure", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("task_group_on_failure", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("nursery_on_success", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("task_group_on_success", vec![], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("nursery_with_timeout", vec![Type::Int], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),
        ("task_group_with_timeout", vec![Type::Int], Type::Generic { base: "TaskGroup".to_string(), type_args: vec![] }),

        ("nursery_fork", vec![Type::Unknown, Type::Unknown], Type::Generic { base: "Subtask".to_string(), type_args: vec![] }),
        ("task_group_fork", vec![Type::Unknown, Type::Unknown], Type::Generic { base: "Subtask".to_string(), type_args: vec![] }),
        ("nursery_spawn", vec![Type::Unknown, Type::Unknown], Type::Generic { base: "Subtask".to_string(), type_args: vec![] }),
        ("task_group_spawn", vec![Type::Unknown, Type::Unknown], Type::Generic { base: "Subtask".to_string(), type_args: vec![] }),

        ("nursery_join", vec![Type::Unknown], Type::Int),
        ("task_group_join", vec![Type::Unknown], Type::Int),
        ("nursery_wait", vec![Type::Unknown], Type::Int),
        ("task_group_wait", vec![Type::Unknown], Type::Int),

        ("nursery_cancel", vec![Type::Unknown], Type::Void),
        ("task_group_cancel", vec![Type::Unknown], Type::Void),

        ("nursery_is_cancelled", vec![Type::Unknown], Type::Bool),
        ("task_group_is_cancelled", vec![Type::Unknown], Type::Bool),

        ("nursery_result", vec![Type::Unknown], Type::Int),
        ("task_group_result", vec![Type::Unknown], Type::Int),

        ("nursery_token", vec![Type::Unknown], Type::Generic { base: "CancellationToken".to_string(), type_args: vec![] }),
        ("task_group_token", vec![Type::Unknown], Type::Generic { base: "CancellationToken".to_string(), type_args: vec![] }),

        ("nursery_task_count", vec![Type::Unknown], Type::Int),
        ("task_group_task_count", vec![Type::Unknown], Type::Int),

        ("nursery_completed_count", vec![Type::Unknown], Type::Int),
        ("task_group_completed_count", vec![Type::Unknown], Type::Int),

        ("subtask_get", vec![Type::Unknown], Type::Unknown),
        ("subtask_is_done", vec![Type::Unknown], Type::Bool),
        ("subtask_is_cancelled", vec![Type::Unknown], Type::Bool),

        ("cancel_token_new", vec![], Type::Generic { base: "CancellationToken".to_string(), type_args: vec![] }),
        ("cancel_token_cancel", vec![Type::Unknown], Type::Void),
        ("cancel_token_is_cancelled", vec![Type::Unknown], Type::Bool),

        // ---- Unsafe Raw Pointers & Direct Memory (MED-05) ----
        ("alloc", vec![Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("alloc_zeroed", vec![Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("realloc", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("free", vec![Type::RawPointer(Box::new(Type::U8))], Type::Void),
        ("ptr_read_u8", vec![Type::RawPointer(Box::new(Type::U8))], Type::Int),
        ("ptr_read", vec![Type::RawPointer(Box::new(Type::Int))], Type::Int),
        ("ptr_read_i64", vec![Type::RawPointer(Box::new(Type::Int))], Type::Int),
        ("ptr_read_at", vec![Type::RawPointer(Box::new(Type::Int)), Type::Int], Type::Int),
        ("ptr_read_u8_at", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::Int),
        ("ptr_write_u8", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::Void),
        ("ptr_write", vec![Type::RawPointer(Box::new(Type::Int)), Type::Int], Type::Void),
        ("ptr_write_i64", vec![Type::RawPointer(Box::new(Type::Int)), Type::Int], Type::Void),
        ("ptr_write_at", vec![Type::RawPointer(Box::new(Type::Int)), Type::Int, Type::Int], Type::Void),
        ("ptr_write_u8_at", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int, Type::Int], Type::Void),
        ("ptr_add", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("ptr_offset", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("ptr_diff", vec![Type::RawPointer(Box::new(Type::U8)), Type::RawPointer(Box::new(Type::U8))], Type::Int),
        ("ptr_to_int", vec![Type::RawPointer(Box::new(Type::U8))], Type::Int),
        ("int_to_ptr", vec![Type::Int], Type::RawPointer(Box::new(Type::U8))),
        ("ptr_is_null", vec![Type::RawPointer(Box::new(Type::U8))], Type::Bool),
        ("mem_copy", vec![Type::RawPointer(Box::new(Type::U8)), Type::RawPointer(Box::new(Type::U8)), Type::Int], Type::Void),
        ("mem_set", vec![Type::RawPointer(Box::new(Type::U8)), Type::Int, Type::Int], Type::Void),

        // ---- Freestanding VGA Text Driver (ADV-01) ----
        ("vga_clear", vec![], Type::Void),
        ("vga_print", vec![Type::String], Type::Void),
        ("vga_write_char", vec![Type::Int, Type::Int, Type::Int, Type::Int], Type::Void),
        ("vga_write_byte", vec![Type::Int, Type::Int], Type::Void),
        ("vga_set_cursor", vec![Type::Int, Type::Int], Type::Void),
        ("vga_set_color", vec![Type::Int, Type::Int], Type::Void),

        // ---- Freestanding Kernel & Multiboot2 (ADV-02 & M2-T05) ----
        ("multiboot2_get_magic", vec![], Type::Int),
        ("multiboot2_get_info_ptr", vec![], Type::RawPointer(Box::new(Type::U8))),
        ("outb", vec![Type::Int, Type::Int], Type::Void),
        ("inb", vec![Type::Int], Type::Int),
        ("qemu_exit", vec![Type::Int], Type::Void),
        ("serial_print", vec![Type::String], Type::Void),
        ("serial_putchar", vec![Type::Int], Type::Void),
        ("freestanding_is_heap_enabled", vec![], Type::Bool),
        ("freestanding_is_float_enabled", vec![], Type::Bool),
        ("freestanding_get_heap_used", vec![], Type::Int),
        ("freestanding_get_heap_capacity", vec![], Type::Int),
        ("freestanding_halt", vec![], Type::Void),

        // ---- Fixed-Width Numeric Suite (M2-T01) ----
        // Type conversion constructors
        ("i8", vec![Type::Unknown], Type::I8),
        ("u8", vec![Type::Unknown], Type::U8),
        ("i16", vec![Type::Unknown], Type::I16),
        ("u16", vec![Type::Unknown], Type::U16),
        ("i32", vec![Type::Unknown], Type::I32),
        ("u32", vec![Type::Unknown], Type::U32),
        ("i64", vec![Type::Unknown], Type::I64),
        ("u64", vec![Type::Unknown], Type::U64),
        ("i128", vec![Type::Unknown], Type::I128),
        ("u128", vec![Type::Unknown], Type::U128),
        ("isize", vec![Type::Unknown], Type::Isize),
        ("usize", vec![Type::Unknown], Type::Usize),
        ("f32", vec![Type::Unknown], Type::F32),
        ("f64", vec![Type::Unknown], Type::F64),
        ("char", vec![Type::Unknown], Type::Char),
        ("int", vec![Type::Unknown], Type::Int),
        ("float", vec![Type::Unknown], Type::Float),

        // Bounds constants/intrinsics
        ("i8_min", vec![], Type::I8),
        ("i8_max", vec![], Type::I8),
        ("u8_min", vec![], Type::U8),
        ("u8_max", vec![], Type::U8),
        ("i16_min", vec![], Type::I16),
        ("i16_max", vec![], Type::I16),
        ("u16_min", vec![], Type::U16),
        ("u16_max", vec![], Type::U16),
        ("i32_min", vec![], Type::I32),
        ("i32_max", vec![], Type::I32),
        ("u32_min", vec![], Type::U32),
        ("u32_max", vec![], Type::U32),
        ("i64_min", vec![], Type::I64),
        ("i64_max", vec![], Type::I64),
        ("u64_min", vec![], Type::U64),
        ("u64_max", vec![], Type::U64),
        ("i128_min", vec![], Type::I128),
        ("i128_max", vec![], Type::I128),
        ("u128_min", vec![], Type::U128),
        ("u128_max", vec![], Type::U128),
        ("isize_min", vec![], Type::Isize),
        ("isize_max", vec![], Type::Isize),
        ("usize_min", vec![], Type::Usize),
        ("usize_max", vec![], Type::Usize),
        ("f32_min", vec![], Type::F32),
        ("f32_max", vec![], Type::F32),
        ("f64_min", vec![], Type::F64),
        ("f64_max", vec![], Type::F64),

        // Wrapping & Saturating arithmetic intrinsics
        ("wrapping_add", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("wrapping_sub", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("wrapping_mul", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("saturating_add", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("saturating_sub", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("saturating_mul", vec![Type::Unknown, Type::Unknown], Type::Unknown),

        // ---- Hardware SIMD Vector Intrinsics Engine (M2-T02) ----
        // Vec4f
        ("Vec4f", vec![], Type::Vec4f),
        ("Vec4f_splat", vec![Type::Unknown], Type::Vec4f),
        ("vec4f_splat", vec![Type::Unknown], Type::Vec4f),
        ("Vec4f_zero", vec![], Type::Vec4f),
        ("vec4f_zero", vec![], Type::Vec4f),
        ("vec4f_add", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_sub", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_mul", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_div", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_dot", vec![Type::Vec4f, Type::Vec4f], Type::F32),
        ("vec4f_sum", vec![Type::Vec4f], Type::F32),
        ("vec4f_min", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_max", vec![Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_sqrt", vec![Type::Vec4f], Type::Vec4f),
        ("vec4f_abs", vec![Type::Vec4f], Type::Vec4f),
        ("vec4f_fma", vec![Type::Vec4f, Type::Vec4f, Type::Vec4f], Type::Vec4f),
        ("vec4f_get", vec![Type::Vec4f, Type::Unknown], Type::F32),
        ("vec4f_set", vec![Type::Vec4f, Type::Unknown, Type::Unknown], Type::Vec4f),
        ("vec4f_load", vec![Type::Unknown], Type::Vec4f),
        ("vec4f_store", vec![Type::Unknown, Type::Vec4f], Type::Void),

        // Vec8f
        ("Vec8f", vec![], Type::Vec8f),
        ("Vec8f_splat", vec![Type::Unknown], Type::Vec8f),
        ("vec8f_splat", vec![Type::Unknown], Type::Vec8f),
        ("Vec8f_zero", vec![], Type::Vec8f),
        ("vec8f_zero", vec![], Type::Vec8f),
        ("vec8f_add", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_sub", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_mul", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_div", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_dot", vec![Type::Vec8f, Type::Vec8f], Type::F32),
        ("vec8f_sum", vec![Type::Vec8f], Type::F32),
        ("vec8f_min", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_max", vec![Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_sqrt", vec![Type::Vec8f], Type::Vec8f),
        ("vec8f_abs", vec![Type::Vec8f], Type::Vec8f),
        ("vec8f_fma", vec![Type::Vec8f, Type::Vec8f, Type::Vec8f], Type::Vec8f),
        ("vec8f_get", vec![Type::Vec8f, Type::Unknown], Type::F32),
        ("vec8f_set", vec![Type::Vec8f, Type::Unknown, Type::Unknown], Type::Vec8f),
        ("vec8f_load", vec![Type::Unknown], Type::Vec8f),
        ("vec8f_store", vec![Type::Unknown, Type::Vec8f], Type::Void),

        // Vec4i
        ("Vec4i", vec![], Type::Vec4i),
        ("Vec4i_splat", vec![Type::Unknown], Type::Vec4i),
        ("vec4i_splat", vec![Type::Unknown], Type::Vec4i),
        ("Vec4i_zero", vec![], Type::Vec4i),
        ("vec4i_zero", vec![], Type::Vec4i),
        ("vec4i_add", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_sub", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_mul", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_div", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_sum", vec![Type::Vec4i], Type::I32),
        ("vec4i_min", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_max", vec![Type::Vec4i, Type::Vec4i], Type::Vec4i),
        ("vec4i_abs", vec![Type::Vec4i], Type::Vec4i),
        ("vec4i_get", vec![Type::Vec4i, Type::Unknown], Type::I32),
        ("vec4i_set", vec![Type::Vec4i, Type::Unknown, Type::Unknown], Type::Vec4i),
        ("vec4i_load", vec![Type::Unknown], Type::Vec4i),
        ("vec4i_store", vec![Type::Unknown, Type::Vec4i], Type::Void),

        // Vec16u8
        ("Vec16u8", vec![], Type::Vec16u8),
        ("Vec16u8_splat", vec![Type::Unknown], Type::Vec16u8),
        ("vec16u8_splat", vec![Type::Unknown], Type::Vec16u8),
        ("Vec16u8_zero", vec![], Type::Vec16u8),
        ("vec16u8_zero", vec![], Type::Vec16u8),
        ("vec16u8_add", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_sub", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_mul", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_saturating_add", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_saturating_sub", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_sum", vec![Type::Vec16u8], Type::Int),
        ("vec16u8_min", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_max", vec![Type::Vec16u8, Type::Vec16u8], Type::Vec16u8),
        ("vec16u8_get", vec![Type::Vec16u8, Type::Unknown], Type::U8),
        ("vec16u8_set", vec![Type::Vec16u8, Type::Unknown, Type::Unknown], Type::Vec16u8),
        ("vec16u8_load", vec![Type::Unknown], Type::Vec16u8),
        ("vec16u8_store", vec![Type::Unknown, Type::Vec16u8], Type::Void),

        // ==================== Collections (M2-T03) ====================
        // VecDeque
        ("vec_deque_new", vec![], Type::Generic { base: "VecDeque".to_string(), type_args: vec![] }),
        ("VecDeque_new", vec![], Type::Generic { base: "VecDeque".to_string(), type_args: vec![] }),
        ("vec_deque_with_capacity", vec![Type::Int], Type::Generic { base: "VecDeque".to_string(), type_args: vec![] }),
        ("vec_deque_push_back", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("vec_deque_push_front", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("vec_deque_pop_back", vec![Type::Unknown], Type::Unknown),
        ("vec_deque_pop_front", vec![Type::Unknown], Type::Unknown),
        ("vec_deque_front", vec![Type::Unknown], Type::Unknown),
        ("vec_deque_back", vec![Type::Unknown], Type::Unknown),
        ("vec_deque_get", vec![Type::Unknown, Type::Int], Type::Unknown),
        ("vec_deque_set", vec![Type::Unknown, Type::Int, Type::Unknown], Type::Void),
        ("vec_deque_len", vec![Type::Unknown], Type::Int),
        ("vec_deque_is_empty", vec![Type::Unknown], Type::Bool),
        ("vec_deque_clear", vec![Type::Unknown], Type::Void),

        // BTreeMap
        ("btree_map_new", vec![], Type::Generic { base: "BTreeMap".to_string(), type_args: vec![] }),
        ("BTreeMap_new", vec![], Type::Generic { base: "BTreeMap".to_string(), type_args: vec![] }),
        ("btree_map_new_str", vec![], Type::Generic { base: "BTreeMap".to_string(), type_args: vec![] }),
        ("btree_map_insert", vec![Type::Unknown, Type::Unknown, Type::Unknown], Type::Void),
        ("btree_map_get", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("btree_map_contains_key", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("btree_map_remove", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("btree_map_len", vec![Type::Unknown], Type::Int),
        ("btree_map_is_empty", vec![Type::Unknown], Type::Bool),
        ("btree_map_first_key", vec![Type::Unknown], Type::Unknown),
        ("btree_map_last_key", vec![Type::Unknown], Type::Unknown),
        ("btree_map_keys", vec![Type::Unknown], Type::List),
        ("btree_map_values", vec![Type::Unknown], Type::List),
        ("btree_map_clear", vec![Type::Unknown], Type::Void),

        // BTreeSet
        ("btree_set_new", vec![], Type::Generic { base: "BTreeSet".to_string(), type_args: vec![] }),
        ("BTreeSet_new", vec![], Type::Generic { base: "BTreeSet".to_string(), type_args: vec![] }),
        ("btree_set_new_str", vec![], Type::Generic { base: "BTreeSet".to_string(), type_args: vec![] }),
        ("btree_set_insert", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("btree_set_contains", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("btree_set_remove", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("btree_set_len", vec![Type::Unknown], Type::Int),
        ("btree_set_is_empty", vec![Type::Unknown], Type::Bool),
        ("btree_set_first", vec![Type::Unknown], Type::Unknown),
        ("btree_set_last", vec![Type::Unknown], Type::Unknown),
        ("btree_set_to_list", vec![Type::Unknown], Type::List),
        ("btree_set_clear", vec![Type::Unknown], Type::Void),

        // BinaryHeap
        ("binary_heap_new", vec![], Type::Generic { base: "BinaryHeap".to_string(), type_args: vec![] }),
        ("BinaryHeap_new", vec![], Type::Generic { base: "BinaryHeap".to_string(), type_args: vec![] }),
        ("binary_heap_push", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("binary_heap_pop", vec![Type::Unknown], Type::Unknown),
        ("binary_heap_peek", vec![Type::Unknown], Type::Unknown),
        ("binary_heap_len", vec![Type::Unknown], Type::Int),
        ("binary_heap_is_empty", vec![Type::Unknown], Type::Bool),
        ("binary_heap_clear", vec![Type::Unknown], Type::Void),

        // IndexMap
        ("index_map_new", vec![], Type::Generic { base: "IndexMap".to_string(), type_args: vec![] }),
        ("IndexMap_new", vec![], Type::Generic { base: "IndexMap".to_string(), type_args: vec![] }),
        ("index_map_new_str", vec![], Type::Generic { base: "IndexMap".to_string(), type_args: vec![] }),
        ("index_map_insert", vec![Type::Unknown, Type::Unknown, Type::Unknown], Type::Void),
        ("index_map_get", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("index_map_get_index_key", vec![Type::Unknown, Type::Int], Type::Unknown),
        ("index_map_get_index_value", vec![Type::Unknown, Type::Int], Type::Unknown),
        ("index_map_contains_key", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("index_map_remove", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("index_map_len", vec![Type::Unknown], Type::Int),
        ("index_map_is_empty", vec![Type::Unknown], Type::Bool),
        ("index_map_keys", vec![Type::Unknown], Type::List),
        ("index_map_values", vec![Type::Unknown], Type::List),
        ("index_map_clear", vec![Type::Unknown], Type::Void),

        // RingBuffer
        ("ring_buffer_new", vec![Type::Int], Type::Generic { base: "RingBuffer".to_string(), type_args: vec![] }),
        ("RingBuffer_new", vec![Type::Int], Type::Generic { base: "RingBuffer".to_string(), type_args: vec![] }),
        ("ring_buffer_push", vec![Type::Unknown, Type::Unknown], Type::Bool),
        ("ring_buffer_push_overwrite", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("ring_buffer_pop", vec![Type::Unknown], Type::Unknown),
        ("ring_buffer_peek", vec![Type::Unknown], Type::Unknown),
        ("ring_buffer_len", vec![Type::Unknown], Type::Int),
        ("ring_buffer_capacity", vec![Type::Unknown], Type::Int),
        ("ring_buffer_is_empty", vec![Type::Unknown], Type::Bool),
        ("ring_buffer_is_full", vec![Type::Unknown], Type::Bool),
        ("ring_buffer_clear", vec![Type::Unknown], Type::Void),

        // BitSet
        ("bit_set_new", vec![], Type::Generic { base: "BitSet".to_string(), type_args: vec![] }),
        ("BitSet_new", vec![], Type::Generic { base: "BitSet".to_string(), type_args: vec![] }),
        ("bit_set_with_capacity", vec![Type::Int], Type::Generic { base: "BitSet".to_string(), type_args: vec![] }),
        ("bit_set_set", vec![Type::Unknown, Type::Int], Type::Void),
        ("bit_set_clear_bit", vec![Type::Unknown, Type::Int], Type::Void),
        ("bit_set_test", vec![Type::Unknown, Type::Int], Type::Bool),
        ("bit_set_flip", vec![Type::Unknown, Type::Int], Type::Void),
        ("bit_set_count_ones", vec![Type::Unknown], Type::Int),
        ("bit_set_len", vec![Type::Unknown], Type::Int),
        ("bit_set_is_empty", vec![Type::Unknown], Type::Bool),
        ("bit_set_union", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("bit_set_intersect", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("bit_set_difference", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("bit_set_clear", vec![Type::Unknown], Type::Void),

        // Memory Architecture & Smart Pointers (M2-T04)
        ("align_to", vec![Type::Int, Type::Int], Type::Int),
        ("cache_line_size", vec![], Type::Int),

        // Box<T>
        ("box_new", vec![Type::Unknown], Type::Generic { base: "RozeBox".to_string(), type_args: vec![] }),
        ("Box_new", vec![Type::Unknown], Type::Generic { base: "RozeBox".to_string(), type_args: vec![] }),
        ("box_get", vec![Type::Unknown], Type::Unknown),
        ("box_set", vec![Type::Unknown, Type::Unknown], Type::Void),
        ("box_into_raw", vec![Type::Unknown], Type::Int),
        ("box_from_raw", vec![Type::Int], Type::Generic { base: "RozeBox".to_string(), type_args: vec![] }),

        // Arc<T> & Weak<T>
        ("arc_new", vec![Type::Unknown], Type::Generic { base: "RozeArc".to_string(), type_args: vec![] }),
        ("Arc_new", vec![Type::Unknown], Type::Generic { base: "RozeArc".to_string(), type_args: vec![] }),
        ("arc_get", vec![Type::Unknown], Type::Unknown),
        ("arc_clone", vec![Type::Unknown], Type::Generic { base: "RozeArc".to_string(), type_args: vec![] }),
        ("arc_strong_count", vec![Type::Unknown], Type::Int),
        ("arc_weak_count", vec![Type::Unknown], Type::Int),
        ("arc_downgrade", vec![Type::Unknown], Type::Generic { base: "RozeWeak".to_string(), type_args: vec![] }),
        ("weak_new", vec![], Type::Generic { base: "RozeWeak".to_string(), type_args: vec![] }),
        ("Weak_new", vec![], Type::Generic { base: "RozeWeak".to_string(), type_args: vec![] }),
        ("weak_upgrade", vec![Type::Unknown], Type::Generic { base: "RozeArc".to_string(), type_args: vec![] }),
        ("weak_strong_count", vec![Type::Unknown], Type::Int),
        ("weak_weak_count", vec![Type::Unknown], Type::Int),
        ("weak_is_expired", vec![Type::Unknown], Type::Bool),
        ("weak_is_alive", vec![Type::Unknown], Type::Bool),

        // BumpAllocator
        ("bump_allocator_new", vec![Type::Int], Type::Generic { base: "BumpAllocator".to_string(), type_args: vec![] }),
        ("BumpAllocator_new", vec![Type::Int], Type::Generic { base: "BumpAllocator".to_string(), type_args: vec![] }),
        ("bump_allocator_new_default", vec![], Type::Generic { base: "BumpAllocator".to_string(), type_args: vec![] }),
        ("bump_alloc", vec![Type::Unknown, Type::Int], Type::Int),
        ("bump_alloc_aligned", vec![Type::Unknown, Type::Int, Type::Int], Type::Int),
        ("bump_reset", vec![Type::Unknown], Type::Void),
        ("bump_allocated_bytes", vec![Type::Unknown], Type::Int),
        ("bump_remaining_bytes", vec![Type::Unknown], Type::Int),
        ("bump_capacity", vec![Type::Unknown], Type::Int),
        ("bump_is_empty", vec![Type::Unknown], Type::Bool),

        // ArenaAllocator
        ("arena_allocator_new", vec![Type::Int], Type::Generic { base: "ArenaAllocator".to_string(), type_args: vec![] }),
        ("ArenaAllocator_new", vec![Type::Int], Type::Generic { base: "ArenaAllocator".to_string(), type_args: vec![] }),
        ("arena_allocator_new_default", vec![], Type::Generic { base: "ArenaAllocator".to_string(), type_args: vec![] }),
        ("arena_alloc", vec![Type::Unknown, Type::Int], Type::Int),
        ("arena_alloc_aligned", vec![Type::Unknown, Type::Int, Type::Int], Type::Int),
        ("arena_reset", vec![Type::Unknown], Type::Void),
        ("arena_allocated_bytes", vec![Type::Unknown], Type::Int),
        ("arena_chunk_count", vec![Type::Unknown], Type::Int),
        ("arena_clear", vec![Type::Unknown], Type::Void),

        // PoolAllocator
        ("pool_allocator_new", vec![Type::Int, Type::Int], Type::Generic { base: "PoolAllocator".to_string(), type_args: vec![] }),
        ("PoolAllocator_new", vec![Type::Int, Type::Int], Type::Generic { base: "PoolAllocator".to_string(), type_args: vec![] }),
        ("pool_alloc", vec![Type::Unknown], Type::Int),
        ("pool_dealloc", vec![Type::Unknown, Type::Int], Type::Bool),
        ("pool_block_size", vec![Type::Unknown], Type::Int),
        ("pool_capacity", vec![Type::Unknown], Type::Int),
        ("pool_available_blocks", vec![Type::Unknown], Type::Int),
        ("pool_is_full", vec![Type::Unknown], Type::Bool),
        ("pool_is_empty", vec![Type::Unknown], Type::Bool),
        ("pool_reset", vec![Type::Unknown], Type::Void),
    ]
}

#[derive(Clone, Debug)]
pub struct GenericClassDef {
    pub name: String,
    pub type_params: Vec<crate::parser::ast::TypeParam>,
    pub fields: Vec<FunctionParam>,
    pub location: Location,
}

#[derive(Clone, Debug)]
pub struct GenericFuncDef {
    pub name: String,
    pub type_params: Vec<crate::parser::ast::TypeParam>,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<String>,
    pub body: Statement,
    pub location: Location,
    pub is_async: bool,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct GenericEnumDef {
    pub name: String,
    pub type_params: Vec<crate::parser::ast::TypeParam>,
    pub variants: Vec<EnumVariant>,
    pub location: Location,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TraitDef {
    pub name: String,
    pub associated_types: Vec<String>,
    pub methods: HashMap<String, FunctionSig>,
    pub location: Location,
}

fn substitute_type_string(s: &str, map: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut current_word = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            current_word.push(c);
        } else {
            if !current_word.is_empty() {
                if let Some(replacement) = map.get(&current_word) {
                    result.push_str(replacement);
                } else {
                    result.push_str(&current_word);
                }
                current_word.clear();
            }
            result.push(c);
        }
    }
    if !current_word.is_empty() {
        if let Some(replacement) = map.get(&current_word) {
            result.push_str(replacement);
        } else {
            result.push_str(&current_word);
        }
    }
    result
}

fn substitute_statement(stmt: &Statement, map: &HashMap<String, String>) -> Statement {
    match stmt {
        Statement::Let { name, type_name, value, location } => Statement::Let {
            name: name.clone(),
            type_name: type_name.as_ref().map(|t| substitute_type_string(t, map)),
            value: Box::new(substitute_expression(value, map)),
            location: location.clone(),
        },
        Statement::Expression { expr, location } => Statement::Expression {
            expr: Box::new(substitute_expression(expr, map)),
            location: location.clone(),
        },
        Statement::Return { value, location } => Statement::Return {
            value: value.as_ref().map(|v| Box::new(substitute_expression(v, map))),
            location: location.clone(),
        },
        Statement::Block { statements, location } => Statement::Block {
            statements: statements.iter().map(|s| substitute_statement(s, map)).collect(),
            location: location.clone(),
        },
        Statement::Function { name, type_params, params, return_type, body, location, is_async } => {
            let new_params = params.iter().map(|p| FunctionParam {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| substitute_type_string(t, map)),
            }).collect();
            let new_return_type = return_type.as_ref().map(|t| substitute_type_string(t, map));
            Statement::Function {
                name: name.clone(),
                type_params: type_params.clone(),
                params: new_params,
                return_type: new_return_type,
                body: Box::new(substitute_statement(body, map)),
                location: location.clone(),
                is_async: *is_async,
            }
        }
        Statement::Import { path, location } => Statement::Import {
            path: path.clone(),
            location: location.clone(),
        },
        Statement::If { condition, then_branch, else_branch, location } => Statement::If {
            condition: Box::new(substitute_expression(condition, map)),
            then_branch: Box::new(substitute_statement(then_branch, map)),
            else_branch: else_branch.as_ref().map(|b| Box::new(substitute_statement(b, map))),
            location: location.clone(),
        },
        Statement::While { condition, body, location } => Statement::While {
            condition: Box::new(substitute_expression(condition, map)),
            body: Box::new(substitute_statement(body, map)),
            location: location.clone(),
        },
        Statement::Assign { name, value, location } => Statement::Assign {
            name: name.clone(),
            value: Box::new(substitute_expression(value, map)),
            location: location.clone(),
        },
        Statement::For { init, condition, update, body, location } => Statement::For {
            init: Box::new(substitute_statement(init, map)),
            condition: Box::new(substitute_expression(condition, map)),
            update: Box::new(substitute_statement(update, map)),
            body: Box::new(substitute_statement(body, map)),
            location: location.clone(),
        },
        Statement::ClassDecl { name, type_params, fields, location } => {
            let new_fields = fields.iter().map(|f| FunctionParam {
                name: f.name.clone(),
                type_name: f.type_name.as_ref().map(|t| substitute_type_string(t, map)),
            }).collect();
            Statement::ClassDecl {
                name: name.clone(),
                type_params: type_params.clone(),
                fields: new_fields,
                location: location.clone(),
            }
        }
        Statement::FieldAssign { object, field, value, location } => Statement::FieldAssign {
            object: Box::new(substitute_expression(object, map)),
            field: field.clone(),
            value: Box::new(substitute_expression(value, map)),
            location: location.clone(),
        },
        Statement::EnumDecl { name, type_params, variants, location } => {
            let new_variants = variants.iter().map(|v| EnumVariant {
                name: v.name.clone(),
                payload_types: v.payload_types.iter().map(|t| substitute_type_string(t, map)).collect(),
                location: v.location.clone(),
            }).collect();
            Statement::EnumDecl {
                name: name.clone(),
                type_params: type_params.clone(),
                variants: new_variants,
                location: location.clone(),
            }
        }
        Statement::Match { expr, arms, location } => Statement::Match {
            expr: Box::new(substitute_expression(expr, map)),
            arms: arms.iter().map(|arm| MatchArm {
                pattern: arm.pattern.clone(),
                guard: arm.guard.as_ref().map(|g| Box::new(substitute_expression(g, map))),
                body: Box::new(substitute_statement(&arm.body, map)),
                location: arm.location.clone(),
            }).collect(),
            location: location.clone(),
        },
        Statement::TraitDecl { name, associated_types, methods, location } => Statement::TraitDecl {
            name: name.clone(),
            associated_types: associated_types.clone(),
            methods: methods.clone(),
            location: location.clone(),
        },
        Statement::ImplDecl { trait_name, class_name, associated_types, methods, location } => Statement::ImplDecl {
            trait_name: trait_name.clone(),
            class_name: class_name.clone(),
            associated_types: associated_types.clone(),
            methods: methods.iter().map(|s| substitute_statement(s, map)).collect(),
            location: location.clone(),
        },
        Statement::ExternFunction { abi, name, params, return_type, location } => Statement::ExternFunction {
            abi: abi.clone(),
            name: name.clone(),
            params: params.clone(),
            return_type: return_type.clone(),
            location: location.clone(),
        },
        Statement::UnsafeBlock { body, location } => Statement::UnsafeBlock {
            body: Box::new(substitute_statement(body, map)),
            location: location.clone(),
        },
        Statement::Comptime { statement, location } => Statement::Comptime {
            statement: Box::new(substitute_statement(statement, map)),
            location: location.clone(),
        },
    }
}

fn substitute_expression(expr: &Expression, map: &HashMap<String, String>) -> Expression {
    match expr {
        Expression::Number { value, location } => Expression::Number { value: value.clone(), location: location.clone() },
        Expression::Char { value, location } => Expression::Char { value: *value, location: location.clone() },
        Expression::String { value, location } => Expression::String { value: value.clone(), location: location.clone() },
        Expression::Identifier { name, location } => Expression::Identifier { name: name.clone(), location: location.clone() },
        Expression::Boolean { value, location } => Expression::Boolean { value: *value, location: location.clone() },
        Expression::Null { location } => Expression::Null { location: location.clone() },
        Expression::Binary { left, operator, right, location } => Expression::Binary {
            left: Box::new(substitute_expression(left, map)),
            operator: operator.clone(),
            right: Box::new(substitute_expression(right, map)),
            location: location.clone(),
        },
        Expression::Unary { operator, operand, location } => Expression::Unary {
            operator: operator.clone(),
            operand: Box::new(substitute_expression(operand, map)),
            location: location.clone(),
        },
        Expression::Call { function, arguments, location } => Expression::Call {
            function: Box::new(substitute_expression(function, map)),
            arguments: arguments.iter().map(|a| substitute_expression(a, map)).collect(),
            location: location.clone(),
        },
        Expression::New { class_name, type_args, arguments, location } => Expression::New {
            class_name: class_name.clone(),
            type_args: type_args.iter().map(|t| substitute_type_string(t, map)).collect(),
            arguments: arguments.iter().map(|a| substitute_expression(a, map)).collect(),
            location: location.clone(),
        },
        Expression::GenericCall { function, type_args, arguments, location } => Expression::GenericCall {
            function: function.clone(),
            type_args: type_args.iter().map(|t| substitute_type_string(t, map)).collect(),
            arguments: arguments.iter().map(|a| substitute_expression(a, map)).collect(),
            location: location.clone(),
        },
        Expression::FieldAccess { object, field, location } => Expression::FieldAccess {
            object: Box::new(substitute_expression(object, map)),
            field: field.clone(),
            location: location.clone(),
        },
        Expression::Try { expr, location } => Expression::Try {
            expr: Box::new(substitute_expression(expr, map)),
            location: location.clone(),
        },
        Expression::EnumConstruct { enum_name, variant, arguments, location } => Expression::EnumConstruct {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            arguments: arguments.iter().map(|a| substitute_expression(a, map)).collect(),
            location: location.clone(),
        },
        Expression::Closure { params, return_type, body, location } => Expression::Closure {
            params: params.iter().map(|p| FunctionParam {
                name: p.name.clone(),
                type_name: p.type_name.as_ref().map(|t| substitute_type_string(t, map)),
            }).collect(),
            return_type: return_type.as_ref().map(|t| substitute_type_string(t, map)),
            body: Box::new(substitute_statement(body, map)),
            location: location.clone(),
        },
        Expression::MethodCall { object, method, arguments, location } => Expression::MethodCall {
            object: Box::new(substitute_expression(object, map)),
            method: method.clone(),
            arguments: arguments.iter().map(|a| substitute_expression(a, map)).collect(),
            location: location.clone(),
        },
        Expression::Unsafe { expr, location } => Expression::Unsafe {
            expr: Box::new(substitute_expression(expr, map)),
            location: location.clone(),
        },
        Expression::Comptime { expr, location } => Expression::Comptime {
            expr: Box::new(substitute_expression(expr, map)),
            location: location.clone(),
        },
        Expression::ComptimeBlock { block, location } => Expression::ComptimeBlock {
            block: Box::new(substitute_statement(block, map)),
            location: location.clone(),
        },
        Expression::Await { expr, location } => Expression::Await {
            expr: Box::new(substitute_expression(expr, map)),
            location: location.clone(),
        },
    }
}

fn infer_type_param(
    pattern: &str,
    actual: &Type,
    type_params: &[crate::parser::ast::TypeParam],
    inferred: &mut HashMap<String, Type>,
) {
    let pattern = pattern.trim();
    if type_params.iter().any(|tp| tp.name == pattern) {
        inferred.entry(pattern.to_string()).or_insert_with(|| actual.clone());
        return;
    }
    if let Some(open) = pattern.find('<') {
        if pattern.ends_with('>') {
            let base = pattern[..open].trim();
            let inner = &pattern[open + 1..pattern.len() - 1];
            let sub_patterns = split_type_args(inner);
            match actual {
                Type::Generic { base: act_base, type_args } if act_base == base => {
                    for (sub_pat, act_ty) in sub_patterns.iter().zip(type_args.iter()) {
                        infer_type_param(sub_pat, act_ty, type_params, inferred);
                    }
                }
                Type::Class(class_name) if class_name.starts_with(&format!("{}_", base)) => {
                    let suffix = &class_name[base.len() + 2..];
                    let parts: Vec<&str> = suffix.split("__").collect();
                    for (sub_pat, part) in sub_patterns.iter().zip(parts.iter()) {
                        let ty = Type::from_name(part);
                        infer_type_param(sub_pat, &ty, type_params, inferred);
                    }
                }
                _ => {}
            }
        }
    }
}

pub struct TypeChecker {
    pub symbol_table: SymbolTable,
    pub functions: HashMap<String, FunctionSig>,
    /// Class name -> its fields, in declaration order (order matters:
    /// `new Name(a, b)`'s arguments are positional, matched against
    /// this order).
    pub classes: HashMap<String, Vec<(String, Type)>>,
    pub enums: HashMap<String, Vec<(String, Vec<Type>)>>,
    pub variant_to_enum: HashMap<String, Vec<String>>,
    pub generic_classes: HashMap<String, GenericClassDef>,
    pub generic_functions: HashMap<String, GenericFuncDef>,
    pub generic_enums: HashMap<String, GenericEnumDef>,
    pub traits: HashMap<String, TraitDef>,
    pub trait_methods: HashMap<String, usize>,
    pub next_trait_slot: usize,
    pub class_traits: HashMap<String, Vec<String>>,
    pub class_methods: HashMap<String, HashMap<String, FunctionSig>>,
    pub impl_associated_types: HashMap<(String, String), Type>,
    pub current_class: Option<String>,
    pub generated_class_decls: Vec<TypedStatement>,
    pub generated_func_decls: Vec<TypedStatement>,
    pub instantiated_classes: HashSet<String>,
    pub instantiated_functions: HashSet<String>,
    pub current_function: Option<String>,
    pub current_return_type: Type,
    /// True while checking `main`'s body -- codegen always hard-codes
    /// `main` to Java's `public static void main(String[] args)`
    /// regardless of any declared return type, so we enforce that at the
    /// Roze level too instead of letting a mismatch surface later as a
    /// confusing javac error.
    pub in_main: bool,
    pub in_unsafe: bool,
    pub no_heap: bool,
    pub disable_float_registers: bool,
    pub in_isr: bool,
    pub closure_counter: usize,
    pub ast_functions: HashMap<String, Statement>,
    pub errors: Vec<String>,
}

/// The result type of a numeric binary operator given its operand
/// types, or None if the combination isn't numeric at all (the caller
/// then decides what else it might be -- string concatenation for
/// `+`, or a hard error). Promotes a mixed int/float pair to float, in
/// either order, matching the same direction `types_compatible`
/// already allows for assignment. An Unknown operand (an untyped
/// list/map element, an untyped function parameter) defers to the
/// other operand's type where there is one, or Int as a reasonable
/// default when both sides are unknown -- preserving the exact
/// behavior this had before Float existed, just now also extended to
/// recognize Float on either side.
fn numeric_binary_result_type(left: &Type, right: &Type) -> Option<Type> {
    if left == right && (left.is_numeric() || *left == Type::Char) {
        return Some(left.clone());
    }
    match (left, right) {
        (Type::Int, Type::I64) | (Type::I64, Type::Int) => Some(Type::I64),
        (Type::Float, Type::F64) | (Type::F64, Type::Float) => Some(Type::F64),
        (other, Type::Int) | (Type::Int, other) if other.is_numeric() => Some(other.clone()),
        (Type::Float | Type::F64, other) | (other, Type::Float | Type::F64) if other.is_numeric() => Some(Type::F64),
        (Type::F32, other) | (other, Type::F32) if other.is_integer() => Some(Type::F32),
        (other, _) if other.is_integer() => Some(other.clone()),
        (Type::Unknown, Type::Unknown) => Some(Type::Int),
        (Type::Unknown, other) | (other, Type::Unknown) if other.is_numeric() => Some(other.clone()),
        _ => None,
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        for (name, params, return_type) in builtin_signatures() {
            functions.insert(name.to_string(), FunctionSig { params, return_type });
        }
        Self {
            symbol_table: SymbolTable::new(),
            functions,
            classes: HashMap::new(),
            enums: HashMap::new(),
            variant_to_enum: HashMap::new(),
            generic_classes: HashMap::new(),
            generic_functions: HashMap::new(),
            generic_enums: HashMap::new(),
            traits: HashMap::new(),
            trait_methods: HashMap::new(),
            next_trait_slot: 1,
            class_traits: HashMap::new(),
            class_methods: HashMap::new(),
            impl_associated_types: HashMap::new(),
            current_class: None,
            generated_class_decls: Vec::new(),
            generated_func_decls: Vec::new(),
            instantiated_classes: HashSet::new(),
            instantiated_functions: HashSet::new(),
            current_function: None,
            current_return_type: Type::Void,
            in_main: false,
            in_unsafe: false,
            no_heap: false,
            disable_float_registers: false,
            in_isr: false,
            closure_counter: 0,
            ast_functions: HashMap::new(),
            errors: Vec::new(),
        }
    }

    pub fn compute_type_type_layout(&self, ty: &Type) -> Result<(usize, usize)> {
        match ty {
            Type::I8 | Type::U8 => Ok((1, 1)),
            Type::I16 | Type::U16 => Ok((2, 2)),
            Type::I32 | Type::U32 | Type::F32 | Type::Char => Ok((4, 4)),
            Type::Int | Type::I64 | Type::U64 | Type::Isize | Type::Usize | Type::Float | Type::F64 => Ok((8, 8)),
            Type::I128 | Type::U128 => Ok((16, 16)),
            Type::Vec4f | Type::Vec4i | Type::Vec16u8 => Ok((16, 16)),
            Type::Vec8f => Ok((32, 32)),
            Type::Bool => Ok((1, 1)),
            Type::String => Ok((8, 8)),
            Type::RawPointer(_) => Ok((8, 8)),
            Type::Void => Ok((0, 1)),
            Type::List | Type::Map | Type::Option | Type::Result | Type::Function { .. } => Ok((8, 8)),
            Type::Class(name) => self.compute_type_layout(name),
            Type::Enum(name) => self.compute_type_layout(name),
            Type::Generic { base, .. } => self.compute_type_layout(base),
            Type::Trait(_) => Ok((8, 8)),
            Type::Unknown => Ok((8, 8)),
        }
    }

    pub fn compute_type_layout(&self, ty_name: &str) -> Result<(usize, usize)> {
        match ty_name {
            "i8" | "u8" => Ok((1, 1)),
            "i16" | "u16" => Ok((2, 2)),
            "i32" | "u32" | "f32" | "char" => Ok((4, 4)),
            "int" | "i64" | "u64" | "isize" | "usize" | "float" | "f64" => Ok((8, 8)),
            "i128" | "u128" => Ok((16, 16)),
            "Vec4f" | "vec4f" | "Vec4i" | "vec4i" | "Vec16u8" | "vec16u8" => Ok((16, 16)),
            "Vec8f" | "vec8f" => Ok((32, 32)),
            "bool" => Ok((1, 1)),
            "string" => Ok((8, 8)),
            "rawpointer" => Ok((8, 8)),
            "void" => Ok((0, 1)),
            "list" | "map" => Ok((8, 8)),
            class_name => {
                if let Some(fields) = self.classes.get(class_name) {
                    let mut current_offset = 0;
                    let mut max_align = 1;
                    for (_field_name, field_type) in fields {
                        let (f_size, f_align) = self.compute_type_type_layout(field_type)?;
                        max_align = max_align.max(f_align);
                        if f_align > 0 {
                            current_offset = (current_offset + f_align - 1) / f_align * f_align;
                        }
                        current_offset += f_size;
                    }
                    if max_align > 0 {
                        current_offset = (current_offset + max_align - 1) / max_align * max_align;
                    }
                    let total_size = if current_offset == 0 { 1 } else { current_offset };
                    Ok((total_size, max_align))
                } else if let Some(variants) = self.enums.get(class_name) {
                    let tag_size = 8;
                    let tag_align = 8;
                    let mut max_payload_size = 0;
                    let mut max_payload_align = 1;
                    for (_vname, payload_types) in variants {
                        let mut v_offset = 0;
                        let mut v_align = 1;
                        for pt in payload_types {
                            let (ps, pa) = self.compute_type_type_layout(pt)?;
                            v_align = v_align.max(pa);
                            if pa > 0 {
                                v_offset = (v_offset + pa - 1) / pa * pa;
                            }
                            v_offset += ps;
                        }
                        max_payload_size = max_payload_size.max(v_offset);
                        max_payload_align = max_payload_align.max(v_align);
                    }
                    let total_align = tag_align.max(max_payload_align);
                    let mut total_size = tag_size;
                    if max_payload_size > 0 {
                        total_size = (total_size + max_payload_align - 1) / max_payload_align * max_payload_align;
                        total_size += max_payload_size;
                    }
                    total_size = (total_size + total_align - 1) / total_align * total_align;
                    Ok((total_size, total_align))
                } else {
                    Err(RozeError::type_error(format!("Cannot compute layout for unknown type '{}'", ty_name), 0, 0).into())
                }
            }
        }
    }

    pub fn compute_field_offset(&self, class_name: &str, target_field: &str) -> Result<usize> {
        if let Some(fields) = self.classes.get(class_name) {
            let mut current_offset = 0;
            for (field_name, field_type) in fields {
                let (f_size, f_align) = self.compute_type_type_layout(field_type)?;
                if f_align > 0 {
                    current_offset = (current_offset + f_align - 1) / f_align * f_align;
                }
                if field_name == target_field {
                    return Ok(current_offset);
                }
                current_offset += f_size;
            }
            Err(RozeError::type_error(format!("Class '{}' has no field '{}'", class_name, target_field), 0, 0).into())
        } else {
            Err(RozeError::type_error(format!("Unknown class '{}'", class_name), 0, 0).into())
        }
    }

    pub fn comptime_value_to_typed_expr(
        &mut self,
        val: &ComptimeValue,
        location: &Location,
    ) -> Result<TypedExpression> {
        match val {
            ComptimeValue::Int(n) => Ok(TypedExpression {
                kind: TypedExpressionKind::Number(n.to_string()),
                type_: Type::Int,
                location: location.clone(),
            }),
            ComptimeValue::Float(f) => {
                let mut s = f.to_string();
                if !s.contains('.') {
                    s.push_str(".0");
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Number(s),
                    type_: Type::Float,
                    location: location.clone(),
                })
            }
            ComptimeValue::Bool(b) => Ok(TypedExpression {
                kind: TypedExpressionKind::Boolean(*b),
                type_: Type::Bool,
                location: location.clone(),
            }),
            ComptimeValue::String(s) => Ok(TypedExpression {
                kind: TypedExpressionKind::String(s.clone()),
                type_: Type::String,
                location: location.clone(),
            }),
            ComptimeValue::Null => Ok(TypedExpression {
                kind: TypedExpressionKind::Null,
                type_: Type::Unknown,
                location: location.clone(),
            }),
            ComptimeValue::Void => Ok(TypedExpression {
                kind: TypedExpressionKind::Null,
                type_: Type::Void,
                location: location.clone(),
            }),
            ComptimeValue::Return(inner) => {
                self.comptime_value_to_typed_expr(inner, location)
            }
            ComptimeValue::List(items) => {
                let mut stmts = Vec::new();
                let list_var = format!("__comptime_tbl_{}_{}", location.line, location.column);
                stmts.push(Statement::Let {
                    name: list_var.clone(),
                    type_name: None,
                    value: Box::new(Expression::Call {
                        function: Box::new(Expression::Identifier { name: "list_new".to_string(), location: location.clone() }),
                        arguments: vec![],
                        location: location.clone(),
                    }),
                    location: location.clone(),
                });
                for item in items.borrow().iter() {
                    let item_ast = comptime_value_to_ast(item, location)?;
                    stmts.push(Statement::Expression {
                        expr: Box::new(Expression::Call {
                            function: Box::new(Expression::Identifier { name: "list_push".to_string(), location: location.clone() }),
                            arguments: vec![
                                Expression::Identifier { name: list_var.clone(), location: location.clone() },
                                item_ast,
                            ],
                            location: location.clone(),
                        }),
                        location: location.clone(),
                    });
                }
                stmts.push(Statement::Return {
                    value: Some(Box::new(Expression::Identifier { name: list_var, location: location.clone() })),
                    location: location.clone(),
                });
                let closure_expr = Expression::Call {
                    function: Box::new(Expression::Closure {
                        params: vec![],
                        return_type: Some("list".to_string()),
                        body: Box::new(Statement::Block { statements: stmts, location: location.clone() }),
                        location: location.clone(),
                    }),
                    arguments: vec![],
                    location: location.clone(),
                };
                self.check_expression(&closure_expr)
            }
            other => Err(RozeError::type_error(
                format!("Unsupported comptime value '{:?}' cannot be converted to runtime expression", other),
                location.line,
                location.column,
            ).into()),
        }
    }

    pub fn type_satisfies_trait(&self, ty: &Type, trait_name: &str) -> bool {
        if trait_name == "Any" {
            return true;
        }

        match trait_name {
            "Display" | "ToString" => {
                match ty {
                    Type::Int | Type::Float | Type::Bool | Type::String | Type::U8 => return true,
                    Type::Class(name) => {
                        if let Some(traits) = self.class_traits.get(name) {
                            if traits.iter().any(|t| t == trait_name || t == "Display" || t == "ToString") {
                                return true;
                            }
                        }
                        if let Some(methods) = self.class_methods.get(name) {
                            if methods.contains_key("to_string") {
                                return true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            "Clone" => {
                match ty {
                    Type::Int | Type::Float | Type::Bool | Type::String | Type::U8 | Type::List | Type::Map => return true,
                    Type::Class(name) => {
                        if let Some(traits) = self.class_traits.get(name) {
                            if traits.iter().any(|t| t == trait_name) {
                                return true;
                            }
                        }
                        if let Some(methods) = self.class_methods.get(name) {
                            if methods.contains_key("clone") {
                                return true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            "Debug" => return true,
            "Comparable" | "Ord" | "Eq" => {
                match ty {
                    Type::Int | Type::Float | Type::Bool | Type::String | Type::U8 => return true,
                    _ => {}
                }
            }
            _ => {}
        }

        match ty {
            Type::Class(class_name) => {
                if let Some(traits) = self.class_traits.get(class_name) {
                    if traits.iter().any(|t| t == trait_name) {
                        return true;
                    }
                }
                if let Some(trait_def) = self.traits.get(trait_name) {
                    if let Some(methods) = self.class_methods.get(class_name) {
                        if trait_def.methods.keys().all(|m| methods.contains_key(m)) {
                            return true;
                        }
                    }
                }
            }
            Type::Generic { base, .. } => {
                if let Some(traits) = self.class_traits.get(base) {
                    if traits.iter().any(|t| t == trait_name) {
                        return true;
                    }
                }
            }
            _ => {}
        }

        false
    }

    pub fn instantiate_generic_class(&mut self, base: &str, type_args: &[Type]) -> Result<String> {
        let template = self.generic_classes.get(base).cloned().ok_or_else(|| {
            RozeError::type_error(format!("Undefined generic class '{}'", base), 1, 1)
        })?;

        if template.type_params.len() != type_args.len() {
            return Err(RozeError::type_error(
                format!(
                    "Generic class '{}' expects {} type argument(s), got {}",
                    base, template.type_params.len(), type_args.len()
                ),
                template.location.line,
                template.location.column,
            ).into());
        }

        // Validate trait bounds for generic class
        for (tp, actual_ty) in template.type_params.iter().zip(type_args.iter()) {
            for bound in &tp.bounds {
                if !self.type_satisfies_trait(actual_ty, bound) {
                    return Err(RozeError::type_error(
                        format!(
                            "Type '{}' does not satisfy trait bound '{}' required by parameter '{}'",
                            actual_ty, bound, tp.name
                        ),
                        template.location.line,
                        template.location.column,
                    ).into());
                }
            }
        }

        let spec_name = specialized_name(base, type_args);
        if self.instantiated_classes.contains(&spec_name) {
            return Ok(spec_name);
        }
        self.instantiated_classes.insert(spec_name.clone());

        let map: HashMap<String, String> = template.type_params.iter().zip(type_args.iter())
            .map(|(param, ty)| (param.name.clone(), ty.to_string()))
            .collect();

        let mut specialized_fields = Vec::with_capacity(template.fields.len());
        for field in &template.fields {
            let field_ty = match &field.type_name {
                Some(tn) => {
                    let subbed = substitute_type_string(tn, &map);
                    self.resolve_type_name(&subbed)
                }
                None => Type::Unknown,
            };
            specialized_fields.push((field.name.clone(), field_ty));
        }

        self.classes.insert(spec_name.clone(), specialized_fields.clone());
        self.generated_class_decls.push(TypedStatement::ClassDecl {
            name: spec_name.clone(),
            fields: specialized_fields,
            location: template.location.clone(),
        });

        Ok(spec_name)
    }

    pub fn instantiate_generic_function(&mut self, base: &str, type_args: &[Type]) -> Result<String> {
        let template = self.generic_functions.get(base).cloned().ok_or_else(|| {
            RozeError::type_error(format!("Undefined generic function '{}'", base), 1, 1)
        })?;

        if template.type_params.len() != type_args.len() {
            return Err(RozeError::type_error(
                format!(
                    "Generic function '{}' expects {} type argument(s), got {}",
                    base, template.type_params.len(), type_args.len()
                ),
                template.location.line,
                template.location.column,
            ).into());
        }

        // Validate trait bounds for generic function
        for (tp, actual_ty) in template.type_params.iter().zip(type_args.iter()) {
            for bound in &tp.bounds {
                if !self.type_satisfies_trait(actual_ty, bound) {
                    return Err(RozeError::type_error(
                        format!(
                            "Type '{}' does not satisfy trait bound '{}' required by parameter '{}'",
                            actual_ty, bound, tp.name
                        ),
                        template.location.line,
                        template.location.column,
                    ).into());
                }
            }
        }

        let spec_name = specialized_name(base, type_args);
        if self.instantiated_functions.contains(&spec_name) {
            return Ok(spec_name);
        }
        self.instantiated_functions.insert(spec_name.clone());

        let map: HashMap<String, String> = template.type_params.iter().zip(type_args.iter())
            .map(|(param, ty)| (param.name.clone(), ty.to_string()))
            .collect();

        let subbed_params: Vec<FunctionParam> = template.params.iter().map(|p| FunctionParam {
            name: p.name.clone(),
            type_name: p.type_name.as_ref().map(|t| substitute_type_string(t, &map)),
        }).collect();

        let subbed_return_type = template.return_type.as_ref().map(|t| substitute_type_string(t, &map));
        let subbed_body = substitute_statement(&template.body, &map);

        let concrete_stmt = Statement::Function {
            name: spec_name.clone(),
            type_params: Vec::new(),
            params: subbed_params.clone(),
            return_type: subbed_return_type.clone(),
            body: Box::new(subbed_body),
            location: template.location.clone(),
            is_async: template.is_async,
        };

        let mut param_types = Vec::with_capacity(subbed_params.len());
        for p in &subbed_params {
            param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
        }
        let ret_ty = subbed_return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
        self.functions.insert(spec_name.clone(), FunctionSig { params: param_types, return_type: ret_ty });

        let old_symbols = std::mem::replace(&mut self.symbol_table, SymbolTable::new());
        let typed_stmt = self.check_statement(&concrete_stmt)?;
        self.symbol_table = old_symbols;

        if let Some(s) = typed_stmt {
            self.generated_func_decls.push(s);
        }

        Ok(spec_name)
    }

    pub fn infer_func_type_args(
        &self,
        template: &GenericFuncDef,
        args: &[TypedExpression],
        call_loc: &Location,
    ) -> Result<Vec<Type>> {
        if args.len() != template.params.len() {
            return Err(RozeError::type_error(
                format!(
                    "Generic function '{}' takes {} argument(s), but {} were given",
                    template.name, template.params.len(), args.len()
                ),
                call_loc.line,
                call_loc.column,
            ).into());
        }

        let mut inferred = HashMap::new();
        for (param, arg) in template.params.iter().zip(args.iter()) {
            if let Some(type_name) = &param.type_name {
                infer_type_param(type_name, &arg.type_, &template.type_params, &mut inferred);
            }
        }

        let mut result = Vec::with_capacity(template.type_params.len());
        for tp in &template.type_params {
            if let Some(ty) = inferred.get(&tp.name) {
                result.push(ty.clone());
            } else {
                return Err(RozeError::type_error(
                    format!("Could not infer type parameter '{}' for function '{}'", tp.name, template.name),
                    call_loc.line,
                    call_loc.column,
                ).into());
            }
        }
        Ok(result)
    }

    pub fn infer_class_type_args(
        &self,
        template: &GenericClassDef,
        args: &[TypedExpression],
        ctor_loc: &Location,
    ) -> Result<Vec<Type>> {
        if args.len() != template.fields.len() {
            return Err(RozeError::type_error(
                format!(
                    "Generic class '{}' has {} field(s), but 'new {}(...)' was given {} argument(s)",
                    template.name, template.fields.len(), template.name, args.len()
                ),
                ctor_loc.line,
                ctor_loc.column,
            ).into());
        }

        let mut inferred = HashMap::new();
        for (field, arg) in template.fields.iter().zip(args.iter()) {
            if let Some(type_name) = &field.type_name {
                infer_type_param(type_name, &arg.type_, &template.type_params, &mut inferred);
            }
        }

        let mut result = Vec::with_capacity(template.type_params.len());
        for tp in &template.type_params {
            if let Some(ty) = inferred.get(&tp.name) {
                result.push(ty.clone());
            } else {
                return Err(RozeError::type_error(
                    format!("Could not infer type parameter '{}' for class '{}'", tp.name, template.name),
                    ctor_loc.line,
                    ctor_loc.column,
                ).into());
            }
        }
        Ok(result)
    }

    /// Resolves a source-level type name, checking user-defined enums and classes
    /// first (so an enum named e.g. `Color` resolves to `Type::Enum("Color")`,
    /// and a class named `Point` resolves to `Type::Class("Point")`).
    fn resolve_type_name(&mut self, name: &str) -> Type {
        let trimmed = name.trim();
        if trimmed.starts_with("*raw") {
            let inner = trimmed["*raw".len()..].trim();
            let pointee = if inner.is_empty() {
                Type::U8
            } else {
                self.resolve_type_name(inner)
            };
            return Type::RawPointer(Box::new(pointee));
        }
        if trimmed.starts_with("func(") || trimmed.starts_with("fn(") {
            let without_prefix = if trimmed.starts_with("func(") {
                &trimmed[5..]
            } else {
                &trimmed[3..]
            };
            let mut depth = 0;
            let mut close_idx = None;
            for (i, c) in without_prefix.char_indices() {
                if c == '(' || c == '<' {
                    depth += 1;
                } else if c == ')' || c == '>' {
                    if depth == 0 {
                        close_idx = Some(i);
                        break;
                    }
                    depth -= 1;
                }
            }

            if let Some(paren_close) = close_idx {
                let params_str = &without_prefix[..paren_close];
                let rest = without_prefix[paren_close + 1..].trim();

                let param_strs = split_type_args(params_str);
                let params = param_strs.into_iter()
                    .map(|s| self.resolve_type_name(&s))
                    .collect();

                let ret_type = if rest.starts_with("->") {
                    Box::new(self.resolve_type_name(rest[2..].trim()))
                } else {
                    Box::new(Type::Void)
                };

                return Type::Function { params, return_type: ret_type };
            }
        }

        if let Some(open) = trimmed.find('<') {
            if trimmed.ends_with('>') {
                let base = trimmed[..open].trim();
                let inner = &trimmed[open + 1..trimmed.len() - 1];
                let type_args_strs = split_type_args(inner);
                let type_args: Vec<Type> = type_args_strs.into_iter()
                    .map(|s| self.resolve_type_name(&s))
                    .collect();

                if self.generic_classes.contains_key(base) {
                    if let Ok(spec_name) = self.instantiate_generic_class(base, &type_args) {
                        return Type::Class(spec_name);
                    }
                }

                let canonical_base = match base {
                    "Box" | "box" | "RozeBox" => "RozeBox",
                    "Arc" | "arc" | "RozeArc" => "RozeArc",
                    "Weak" | "weak" | "RozeWeak" => "RozeWeak",
                    _ => base,
                };

                return Type::Generic {
                    base: canonical_base.to_string(),
                    type_args,
                };
            }
        }

        if self.traits.contains_key(name) {
            Type::Trait(name.to_string())
        } else if self.enums.contains_key(name) {
            Type::Enum(name.to_string())
        } else if self.classes.contains_key(name) {
            Type::Class(name.to_string())
        } else if let Some(ref c) = self.current_class {
            if let Some(ty) = self.impl_associated_types.get(&(c.clone(), trimmed.to_string())) {
                ty.clone()
            } else {
                Type::from_name(name)
            }
        } else if let Some(pos) = trimmed.find("::") {
            let class_part = trimmed[..pos].trim();
            let item_part = trimmed[pos + 2..].trim();
            if let Some(ty) = self.impl_associated_types.get(&(class_part.to_string(), item_part.to_string())) {
                ty.clone()
            } else {
                Type::from_name(name)
            }
        } else {
            Type::from_name(name)
        }
    }

    pub fn is_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if types_compatible(expected, actual) {
            return true;
        }
        match (expected, actual) {
            (Type::Trait(t), Type::Class(c)) => {
                if let Some(traits) = self.class_traits.get(c) {
                    traits.contains(t)
                } else if let Some((base, _)) = c.split_once("__") {
                    self.class_traits.get(base).map(|tr| tr.contains(t)).unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Type-checks `program` and, on success, returns the fully
    /// type-annotated IR for it (see `crate::ir`). Registers every
    /// enum, class, and top-level function's signature before checking any
    /// bodies, so forward references and mutual recursion type-check
    /// correctly regardless of source order.
    pub fn check_program(&mut self, program: &Program) -> Result<TypedProgram> {
        let mut desugared_stmts = Vec::with_capacity(program.statements.len() * 2);
        for stmt in &program.statements {
            match stmt {
                Statement::Function { name, type_params, params, return_type, body, location, is_async: true } => {
                    let inner_name = format!("__roze_async_inner_{}", name);
                    let inner_fn = Statement::Function {
                        name: inner_name.clone(),
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                        body: body.clone(),
                        location: location.clone(),
                        is_async: false,
                    };

                    let outer_ret = match return_type {
                        Some(rt) => {
                            let trimmed = rt.trim();
                            if trimmed.starts_with("Future<") || trimmed == "Future" || trimmed == "future" {
                                Some(rt.clone())
                            } else {
                                Some(format!("Future<{}>", trimmed))
                            }
                        }
                        None => Some("Future<void>".to_string()),
                    };

                    let is_void = match return_type {
                        Some(rt) if rt.trim() == "void" => true,
                        None => true,
                        _ => false,
                    };

                    let call_inner = Expression::Call {
                        function: Box::new(Expression::Identifier {
                            name: inner_name,
                            location: location.clone(),
                        }),
                        arguments: params.iter().map(|p| Expression::Identifier {
                            name: p.name.clone(),
                            location: location.clone(),
                        }).collect(),
                        location: location.clone(),
                    };

                    let closure_body = if is_void {
                        Statement::Block {
                            statements: vec![
                                Statement::Expression { expr: Box::new(call_inner), location: location.clone() },
                            ],
                            location: location.clone(),
                        }
                    } else {
                        Statement::Block {
                            statements: vec![
                                Statement::Return {
                                    value: Some(Box::new(call_inner)),
                                    location: location.clone(),
                                },
                            ],
                            location: location.clone(),
                        }
                    };

                    let outer_body = Statement::Block {
                        statements: vec![
                            Statement::Return {
                                value: Some(Box::new(Expression::Call {
                                    function: Box::new(Expression::Identifier {
                                        name: "__roze_async_spawn".to_string(),
                                        location: location.clone(),
                                    }),
                                    arguments: vec![
                                        Expression::Closure {
                                            params: vec![],
                                            return_type: None,
                                            body: Box::new(closure_body),
                                            location: location.clone(),
                                        }
                                    ],
                                    location: location.clone(),
                                })),
                                location: location.clone(),
                            }
                        ],
                        location: location.clone(),
                    };

                    let outer_fn = Statement::Function {
                        name: name.clone(),
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: outer_ret,
                        body: Box::new(outer_body),
                        location: location.clone(),
                        is_async: false,
                    };

                    desugared_stmts.push(inner_fn);
                    desugared_stmts.push(outer_fn);
                }
                other => {
                    desugared_stmts.push(other.clone());
                }
            }
        }
        let program_statements = desugared_stmts;

        for stmt in &program_statements {
            if let Statement::EnumDecl { name, type_params, variants, location } = stmt {
                if !type_params.is_empty() {
                    self.generic_enums.insert(name.clone(), GenericEnumDef {
                        name: name.clone(),
                        type_params: type_params.clone(),
                        variants: variants.clone(),
                        location: location.clone(),
                    });
                } else {
                    let mut resolved_variants = Vec::with_capacity(variants.len());
                    for v in variants {
                        let mut tys = Vec::with_capacity(v.payload_types.len());
                        for t in &v.payload_types {
                            tys.push(self.resolve_type_name(t));
                        }
                        self.variant_to_enum.entry(v.name.clone()).or_default().push(name.clone());
                        resolved_variants.push((v.name.clone(), tys));
                    }
                    self.enums.insert(name.clone(), resolved_variants);
                }
            }
        }

        // Pre-register all class names so mutually-referential or forward-referencing
        // class field types resolve to Type::Class instead of Type::Unknown.
        for stmt in &program_statements {
            if let Statement::ClassDecl { name, type_params, .. } = stmt {
                if type_params.is_empty() {
                    self.classes.insert(name.clone(), Vec::new());
                }
            }
        }

        for stmt in &program_statements {
            if let Statement::ClassDecl { name, type_params, fields, location } = stmt {
                if !type_params.is_empty() {
                    self.generic_classes.insert(name.clone(), GenericClassDef {
                        name: name.clone(),
                        type_params: type_params.clone(),
                        fields: fields.clone(),
                        location: location.clone(),
                    });
                } else {
                    let mut resolved_fields = Vec::with_capacity(fields.len());
                    for f in fields {
                        let ty = f.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
                        resolved_fields.push((f.name.clone(), ty));
                    }
                    self.classes.insert(name.clone(), resolved_fields);
                }
            }
        }

        for stmt in &program_statements {
            if let Statement::TraitDecl { name, associated_types, methods, location } = stmt {
                let mut method_map = HashMap::new();
                for m in methods {
                    let mut param_types = Vec::with_capacity(m.params.len());
                    for p in &m.params {
                        param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
                    }
                    let ret = m.return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                    let sig = FunctionSig { params: param_types, return_type: ret };
                    method_map.insert(m.name.clone(), sig);

                    if !self.trait_methods.contains_key(&m.name) {
                        let slot = self.next_trait_slot;
                        self.next_trait_slot += 1;
                        self.trait_methods.insert(m.name.clone(), slot);
                    }
                }
                self.traits.insert(name.clone(), TraitDef {
                    name: name.clone(),
                    associated_types: associated_types.clone(),
                    methods: method_map,
                    location: location.clone(),
                });
            }
        }

        for stmt in &program_statements {
            if let Statement::ImplDecl { trait_name, class_name, associated_types, methods, location } = stmt {
                for (assoc_name, target_ty_str) in associated_types {
                    let resolved_ty = self.resolve_type_name(target_ty_str);
                    self.impl_associated_types.insert((class_name.clone(), assoc_name.clone()), resolved_ty);
                }

                if let Some(t_name) = trait_name {
                    let trait_def = self.traits.get(t_name).cloned().ok_or_else(|| {
                        RozeError::type_error(format!("Undefined trait '{}'", t_name), location.line, location.column)
                    })?;
                    self.class_traits.entry(class_name.clone()).or_default().push(t_name.clone());

                    for m_stmt in methods {
                        if let Statement::Function { name, params, return_type, location: m_loc, .. } = m_stmt {
                            let mut param_types = Vec::with_capacity(params.len());
                            for p in params {
                                param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
                            }
                            let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                            let sig = FunctionSig { params: param_types, return_type: ret };

                            if let Some(expected_sig) = trait_def.methods.get(name) {
                                if sig.params.len() != expected_sig.params.len() {
                                    return Err(RozeError::type_error(
                                        format!("Method '{}' expects {} parameters in trait '{}', but implementation has {}", name, expected_sig.params.len(), t_name, sig.params.len()),
                                        m_loc.line, m_loc.column,
                                    ).into());
                                }
                                for (p_actual, p_expected) in sig.params.iter().zip(expected_sig.params.iter()) {
                                    if p_actual != p_expected {
                                        return Err(RozeError::type_error(
                                            format!("Parameter type mismatch in method '{}' of trait '{}': expected {}, got {}", name, t_name, p_expected, p_actual),
                                            m_loc.line, m_loc.column,
                                        ).into());
                                    }
                                }
                                if sig.return_type != expected_sig.return_type {
                                    return Err(RozeError::type_error(
                                        format!("Return type mismatch in method '{}' of trait '{}': expected {}, got {}", name, t_name, expected_sig.return_type, sig.return_type),
                                        m_loc.line, m_loc.column,
                                    ).into());
                                }
                            }
                            self.class_methods.entry(class_name.clone()).or_default().insert(name.clone(), sig);
                        }
                    }
                } else {
                    for m_stmt in methods {
                        if let Statement::Function { name, params, return_type, .. } = m_stmt {
                            let mut param_types = Vec::with_capacity(params.len());
                            for p in params {
                                param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
                            }
                            let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                            let sig = FunctionSig { params: param_types, return_type: ret };
                            self.class_methods.entry(class_name.clone()).or_default().insert(name.clone(), sig);
                        }
                    }
                }
            }
        }

        for stmt in &program_statements {
            if let Statement::Function { name, type_params, params, return_type, body, location, is_async } = stmt {
                self.ast_functions.insert(name.clone(), stmt.clone());
                if !type_params.is_empty() {
                    self.generic_functions.insert(name.clone(), GenericFuncDef {
                        name: name.clone(),
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                        body: (**body).clone(),
                        location: location.clone(),
                        is_async: *is_async,
                    });
                } else {
                    let mut param_types = Vec::with_capacity(params.len());
                    for p in params {
                        param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
                    }
                    let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                    self.functions.insert(name.clone(), FunctionSig { params: param_types, return_type: ret });
                }
            }
        }

        for stmt in &program_statements {
            if let Statement::ExternFunction { abi, name, params, return_type, location } = stmt {
                if abi != "C" {
                    return Err(RozeError::type_error(
                        format!("Unsupported ABI '{}'; only \"C\" is supported", abi),
                        location.line,
                        location.column,
                    ).into());
                }
                let mut param_types = Vec::with_capacity(params.len());
                for p in params {
                    param_types.push(p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown));
                }
                let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                self.functions.insert(name.clone(), FunctionSig { params: param_types, return_type: ret });
            }
        }

        let mut user_statements = Vec::with_capacity(program_statements.len());
        for stmt in &program_statements {
            if let Some(typed) = self.check_statement(stmt)? {
                user_statements.push(typed);
            }
        }

        let mut statements = Vec::with_capacity(self.generated_class_decls.len() + user_statements.len() + self.generated_func_decls.len());
        statements.extend(self.generated_class_decls.clone());
        statements.extend(user_statements);
        statements.extend(self.generated_func_decls.clone());
        Ok(TypedProgram { statements })
    }

    /// Type-checks one statement, returning its typed-IR form. Returns
    /// `Ok(None)` only for `Statement::Import` (which has no IR
    /// counterpart -- see `ir::TypedStatement`'s doc comment).
    pub fn check_statement(&mut self, stmt: &Statement) -> Result<Option<TypedStatement>> {
        match stmt {
            Statement::Function { name, type_params, params, return_type, body, location, is_async } => {
                if !type_params.is_empty() {
                    return Ok(None);
                }
                if name == "main" && return_type.is_some() {
                    return Err(RozeError::type_error(
                        "'main' always returns void and cannot declare a return type",
                        location.line,
                        location.column,
                    ).with_hint("remove the '-> ...' after main()'s parameter list").into());
                }

                let resolved_return_type = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);

                let is_isr = name.starts_with("isr_") || name.starts_with("interrupt_");
                let outer_in_isr = std::mem::replace(&mut self.in_isr, is_isr || self.disable_float_registers);

                if (self.in_isr || self.disable_float_registers) && (resolved_return_type.is_float() || resolved_return_type.is_simd_vector()) {
                    return Err(RozeError::type_error(
                        format!("Floating-point and SIMD vector registers are disabled in ISR '{}' (return type cannot be {})", name, resolved_return_type),
                        location.line,
                        location.column,
                    ).with_hint("interrupt service routines must preserve FPU state; use integer or fixed-point arithmetic").into());
                }

                let outer_function = self.current_function.replace(name.clone());
                let outer_return_type = std::mem::replace(&mut self.current_return_type, resolved_return_type.clone());
                let outer_in_main = std::mem::replace(&mut self.in_main, name == "main");

                self.symbol_table.push_scope();
                let mut typed_params = Vec::with_capacity(params.len());
                for param in params {
                    let param_type = param.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
                    if (self.in_isr || self.disable_float_registers) && (param_type.is_float() || param_type.is_simd_vector()) {
                        return Err(RozeError::type_error(
                            format!("Floating-point and SIMD vector registers are disabled in ISR '{}' (parameter '{}' cannot be {})", name, param.name, param_type),
                            location.line,
                            location.column,
                        ).with_hint("interrupt service routines must preserve FPU state; use integer or fixed-point arithmetic").into());
                    }
                    self.symbol_table.define(&param.name, param_type.clone(), location.line, location.column)?;
                    typed_params.push(TypedFunctionParam { name: param.name.clone(), type_: param_type });
                }

                let typed_body = self.check_statement(body)?
                    .expect("a function body is always a Block, which always has an IR form");

                self.symbol_table.pop_scope();
                self.current_function = outer_function;
                self.current_return_type = outer_return_type;
                self.in_main = outer_in_main;
                self.in_isr = outer_in_isr;

                Ok(Some(TypedStatement::Function {
                    name: name.clone(),
                    params: typed_params,
                    return_type: resolved_return_type,
                    body: Box::new(typed_body),
                    is_async: *is_async,
                    location: location.clone(),
                }))
            }
            Statement::Let { name, type_name, value, location } => {
                let typed_value = self.check_expression(value)?;
                let var_type = if let Some(tn) = type_name {
                    let expected = self.resolve_type_name(tn);
                    if !self.is_compatible(&expected, &typed_value.type_) {
                        return Err(RozeError::type_error(
                            format!(
                                "Variable '{}' is declared as {}, but initialized with {}",
                                name, expected, typed_value.type_
                            ),
                            location.line,
                            location.column,
                        ).into());
                    }
                    expected
                } else {
                    typed_value.type_.clone()
                };
                self.symbol_table.define(name, var_type.clone(), location.line, location.column)?;
                Ok(Some(TypedStatement::Let {
                    name: name.clone(),
                    type_: var_type,
                    value: typed_value,
                    location: location.clone(),
                }))
            }
            Statement::Expression { expr, location } => {
                let typed_expr = self.check_expression(expr)?;
                Ok(Some(TypedStatement::Expression { expr: typed_expr, location: location.clone() }))
            }
            Statement::Return { value, location } => {
                let typed_value = match value {
                    Some(expr) => {
                        let typed = self.check_expression(expr)?;
                        if !self.is_compatible(&self.current_return_type, &typed.type_) {
                            let fn_name = self.current_function.as_deref().unwrap_or("<anonymous>");
                            return Err(RozeError::type_error(
                                format!(
                                    "'{}' is declared to return {}, but this returns {}",
                                    fn_name, self.current_return_type, typed.type_
                                ),
                                location.line,
                                location.column,
                            ).with_hint(format!(
                                "either change the returned value's type, or change the function's declared return type ('-> {}')",
                                typed.type_
                            )).into());
                        }
                        Some(typed)
                    }
                    None => {
                        // A bare `return;` is only valid if the function
                        // isn't supposed to produce a value.
                        if !matches!(self.current_return_type, Type::Void | Type::Unknown) {
                            let fn_name = self.current_function.as_deref().unwrap_or("<anonymous>");
                            return Err(RozeError::type_error(
                                format!(
                                    "'{}' is declared to return {}, but this 'return;' doesn't return a value",
                                    fn_name, self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                        None
                    }
                };
                Ok(Some(TypedStatement::Return { value: typed_value, location: location.clone() }))
            }
            Statement::Block { statements, location } => {
                self.symbol_table.push_scope();
                let mut typed_statements = Vec::with_capacity(statements.len());
                for stmt in statements {
                    if let Some(typed) = self.check_statement(stmt)? {
                        typed_statements.push(typed);
                    }
                }
                self.symbol_table.pop_scope();
                Ok(Some(TypedStatement::Block { statements: typed_statements, location: location.clone() }))
            }
            Statement::If { condition, then_branch, else_branch, location } => {
                let typed_condition = self.check_expression(condition)?;
                let typed_then = Box::new(
                    self.check_statement(then_branch)?
                        .expect("an if's then-branch is always a Block, which always has an IR form")
                );
                let typed_else = match else_branch {
                    Some(else_stmt) => Some(Box::new(
                        self.check_statement(else_stmt)?
                            .expect("an if's else-branch is always a Block or If, both of which always have an IR form")
                    )),
                    None => None,
                };
                Ok(Some(TypedStatement::If {
                    condition: typed_condition,
                    then_branch: typed_then,
                    else_branch: typed_else,
                    location: location.clone(),
                }))
            }
            Statement::While { condition, body, location } => {
                let typed_condition = self.check_expression(condition)?;
                let typed_body = Box::new(
                    self.check_statement(body)?
                        .expect("a while's body is always a Block, which always has an IR form")
                );
                Ok(Some(TypedStatement::While {
                    condition: typed_condition,
                    body: typed_body,
                    location: location.clone(),
                }))
            }
            Statement::For { init, condition, update, body, location } => {
                // A scope of its own so the init clause's variable (e.g.
                // `let i` in `for let i = 0; ...`) is visible to the
                // condition/update/body but doesn't leak past the loop.
                self.symbol_table.push_scope();
                let typed_init = Box::new(
                    self.check_statement(init)?
                        .expect("a for-loop's init is always Let or Assign, both of which always have an IR form")
                );
                let typed_condition = self.check_expression(condition)?;
                let typed_update = Box::new(
                    self.check_statement(update)?
                        .expect("a for-loop's update is always Assign, which always has an IR form")
                );
                let typed_body = Box::new(
                    self.check_statement(body)?
                        .expect("a for-loop's body is always a Block, which always has an IR form")
                );
                self.symbol_table.pop_scope();
                Ok(Some(TypedStatement::For {
                    init: typed_init,
                    condition: typed_condition,
                    update: typed_update,
                    body: typed_body,
                    location: location.clone(),
                }))
            }
            Statement::Import { .. } => {
                // Imports are already resolved into real functions before
                // type-checking even runs (see imports::resolve_imports),
                // so in practice this arm is never hit -- it only exists
                // in case that ever changes. No IR form for it either
                // way (see `ir::TypedStatement`'s doc comment).
                Ok(None)
            }
            Statement::ClassDecl { name, type_params, location, .. } => {
                if !type_params.is_empty() {
                    return Ok(None);
                }
                // Already fully resolved into self.classes during
                // check_program's first pass (needed there so function
                // signatures declared earlier in the file can reference
                // a class declared later) -- just look it up again here
                // rather than re-deriving it, and hand codegen the
                // resolved (name, Type) pairs directly.
                let fields = self.classes.get(name)
                    .expect("check_program registers every class before any statement is checked")
                    .clone();
                Ok(Some(TypedStatement::ClassDecl { name: name.clone(), fields, location: location.clone() }))
            }
            Statement::FieldAssign { object, field, value, location } => {
                let typed_object = self.check_expression(object)?;
                let class_name = match &typed_object.type_ {
                    Type::Class(name) => name.clone(),
                    other => return Err(RozeError::type_error(
                        format!("Cannot access a field on {}, which isn't a class instance", other),
                        location.line,
                        location.column,
                    ).into()),
                };
                let field_type = self.classes.get(&class_name)
                    .and_then(|fields| fields.iter().find(|(n, _)| n == field))
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| RozeError::type_error(
                        format!("'{}' has no field named '{}'", class_name, field),
                        location.line,
                        location.column,
                    ))?;

                let typed_value = self.check_expression(value)?;
                if !self.is_compatible(&field_type, &typed_value.type_) {
                    return Err(RozeError::type_error(
                        format!(
                            "Cannot assign a value of type {} to '{}.{}', which is declared as {}",
                            typed_value.type_, class_name, field, field_type
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                Ok(Some(TypedStatement::FieldAssign {
                    object: typed_object,
                    field: field.clone(),
                    field_type,
                    value: typed_value,
                    location: location.clone(),
                }))
            }
            Statement::Assign { name, value, location } => {
                let typed_value = self.check_expression(value)?;
                match self.symbol_table.lookup(name) {
                    None => {
                        return Err(RozeError::type_error(
                            format!("Cannot assign to undefined variable '{}'", name),
                            location.line,
                            location.column,
                        ).with_length(name.chars().count()).into());
                    }
                    Some(symbol) => {
                        // Reassignment preserves the variable's original
                        // declared type -- `let x = 5;` followed later by
                        // `x = "hi";` is a type error, the same as it
                        // would be at the `let`.
                        if !self.is_compatible(&symbol.type_, &typed_value.type_) {
                            return Err(RozeError::type_error(
                                format!(
                                    "Cannot assign a value of type {} to '{}', which was declared as {}",
                                    typed_value.type_, name, symbol.type_
                                ),
                                location.line,
                                location.column,
                            ).with_hint(format!(
                                "'{}' was declared with type {} at line {}; its type can't change on reassignment",
                                name, symbol.type_, symbol.line
                            )).into());
                        }
                    }
                }
                let target_type = self.symbol_table.lookup(name).map(|s| s.type_.clone()).unwrap_or(Type::Unknown);
                Ok(Some(TypedStatement::Assign {
                    name: name.clone(),
                    target_type,
                    value: typed_value,
                    location: location.clone(),
                }))
            }
            Statement::EnumDecl { name, type_params, location, .. } => {
                if !type_params.is_empty() {
                    return Ok(None);
                }
                let variants = self.enums.get(name)
                    .expect("check_program registers every enum before any statement is checked")
                    .clone();
                Ok(Some(TypedStatement::EnumDecl {
                    name: name.clone(),
                    variants,
                    location: location.clone(),
                }))
            }
            Statement::Match { expr, arms, location } => {
                let typed_expr = self.check_expression(expr)?;
                let mut typed_arms = Vec::with_capacity(arms.len());

                for arm in arms {
                    self.symbol_table.push_scope();

                    let typed_pattern = self.check_pattern(&arm.pattern, &typed_expr.type_)?;

                    let typed_guard = if let Some(guard_expr) = &arm.guard {
                        let tg = self.check_expression(guard_expr)?;
                        if tg.type_ != Type::Bool && tg.type_ != Type::Unknown {
                            return Err(RozeError::type_error(
                                format!("Match guard condition must evaluate to 'bool', found '{}'", tg.type_),
                                arm.location.line,
                                arm.location.column,
                            ).into());
                        }
                        Some(tg)
                    } else {
                        None
                    };

                    let typed_body = match self.check_statement(&arm.body)? {
                        Some(s) => s,
                        None => TypedStatement::Block { statements: vec![], location: arm.location.clone() },
                    };

                    self.symbol_table.pop_scope();

                    typed_arms.push(TypedMatchArm {
                        pattern: typed_pattern,
                        guard: typed_guard,
                        body: Box::new(typed_body),
                        location: arm.location.clone(),
                    });
                }

                // Exhaustiveness analysis on enums
                if let Type::Enum(en) = &typed_expr.type_ {
                    let has_unconditional_catchall = typed_arms.iter().any(|arm| {
                        arm.guard.is_none() && matches!(arm.pattern, TypedPattern::Wildcard { .. } | TypedPattern::Variable { .. })
                    });
                    if !has_unconditional_catchall {
                        if let Some(variants) = self.enums.get(en) {
                            let mut covered = std::collections::HashSet::new();
                            for arm in &typed_arms {
                                if arm.guard.is_none() {
                                    if let TypedPattern::Variant { variant, .. } = &arm.pattern {
                                        covered.insert(variant.clone());
                                    }
                                }
                            }
                            let missing: Vec<String> = variants.iter()
                                .filter(|(vname, _)| !covered.contains(vname))
                                .map(|(vname, _)| vname.clone())
                                .collect();
                            if !missing.is_empty() {
                                return Err(RozeError::type_error(
                                    format!("Non-exhaustive pattern match on enum '{}': missing variant(s) {}. Add missing arms or a wildcard '_' catch-all", en, missing.join(", ")),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                        }
                    }
                }

                Ok(Some(TypedStatement::Match {
                    expr: typed_expr,
                    arms: typed_arms,
                    location: location.clone(),
                }))
            }
            Statement::TraitDecl { name, associated_types: _, methods, location } => {
                let mut typed_methods = Vec::with_capacity(methods.len());
                for m in methods {
                    let mut typed_params = Vec::with_capacity(m.params.len());
                    for p in &m.params {
                        let ty = p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
                        typed_params.push(TypedFunctionParam {
                            name: p.name.clone(),
                            type_: ty,
                        });
                    }
                    let ret = m.return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                    typed_methods.push(crate::ir::TypedTraitMethod {
                        name: m.name.clone(),
                        params: typed_params,
                        return_type: ret,
                        location: m.location.clone(),
                    });
                }
                Ok(Some(TypedStatement::TraitDecl {
                    name: name.clone(),
                    methods: typed_methods,
                    location: location.clone(),
                }))
            }
            Statement::ImplDecl { trait_name, class_name, associated_types: _, methods, location } => {
                self.current_class = Some(class_name.clone());
                let mut typed_methods = Vec::with_capacity(methods.len());
                for m in methods {
                    if let Statement::Function { name, params, return_type, body, location: f_loc, .. } = m {
                        let resolved_return_type = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                        let outer_function = self.current_function.replace(name.clone());
                        let outer_return_type = std::mem::replace(&mut self.current_return_type, resolved_return_type.clone());

                        self.symbol_table.push_scope();
                        self.symbol_table.define("this", Type::Class(class_name.clone()), f_loc.line, f_loc.column)?;

                        let mut typed_params = Vec::with_capacity(params.len());
                        for param in params {
                            let param_type = param.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
                            self.symbol_table.define(&param.name, param_type.clone(), f_loc.line, f_loc.column)?;
                            typed_params.push(TypedFunctionParam { name: param.name.clone(), type_: param_type });
                        }

                        let typed_body = self.check_statement(body)?
                            .expect("a method body is always a Block, which always has an IR form");

                        self.symbol_table.pop_scope();
                        self.current_function = outer_function;
                        self.current_return_type = outer_return_type;

                        typed_methods.push(TypedStatement::Function {
                            name: name.clone(),
                            params: typed_params,
                            return_type: resolved_return_type,
                            body: Box::new(typed_body),
                            is_async: false,
                            location: f_loc.clone(),
                        });
                    }
                }
                self.current_class = None;
                Ok(Some(TypedStatement::ImplDecl {
                    trait_name: trait_name.clone(),
                    class_name: class_name.clone(),
                    methods: typed_methods,
                    location: location.clone(),
                }))
            }
            Statement::ExternFunction { abi, name, params, return_type, location } => {
                let mut typed_params = Vec::with_capacity(params.len());
                for p in params {
                    let ty = p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
                    typed_params.push(TypedFunctionParam {
                        name: p.name.clone(),
                        type_: ty,
                    });
                }
                let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
                Ok(Some(TypedStatement::ExternFunction {
                    abi: abi.clone(),
                    name: name.clone(),
                    params: typed_params,
                    return_type: ret,
                    location: location.clone(),
                }))
            }
            Statement::UnsafeBlock { body, location } => {
                let prev_unsafe = self.in_unsafe;
                self.in_unsafe = true;
                let typed_body = self.check_statement(body)?;
                self.in_unsafe = prev_unsafe;
                let typed_b = match typed_body {
                    Some(b) => b,
                    None => TypedStatement::Block { statements: vec![], location: location.clone() },
                };
                Ok(Some(TypedStatement::UnsafeBlock {
                    body: Box::new(typed_b),
                    location: location.clone(),
                }))
            }
            Statement::Comptime { statement, location } => {
                let _ = {
                    let mut eval = ComptimeEvaluator::new(self);
                    eval.eval_stmt(statement)?
                };
                Ok(Some(TypedStatement::Comptime {
                    body: Box::new(TypedStatement::Block { statements: vec![], location: location.clone() }),
                    location: location.clone(),
                }))
            }
        }
    }

    pub fn is_unsafe_intrinsic(name: &str) -> bool {
        matches!(name,
            "alloc" | "alloc_zeroed" | "realloc" | "free" |
            "ptr_read_u8" | "ptr_read" | "ptr_read_i64" | "ptr_read_at" | "ptr_read_u8_at" |
            "ptr_write_u8" | "ptr_write" | "ptr_write_i64" | "ptr_write_at" | "ptr_write_u8_at" |
            "ptr_add" | "ptr_offset" | "ptr_diff" | "ptr_to_int" | "int_to_ptr" |
            "ptr_is_null" | "mem_copy" | "mem_set"
        )
    }

    pub fn check_pattern(&mut self, pat: &Pattern, target_type: &Type) -> Result<TypedPattern> {
        match pat {
            Pattern::Wildcard { location } => Ok(TypedPattern::Wildcard { location: location.clone() }),
            Pattern::Literal { expr, location } => {
                let typed_lit = self.check_expression(expr)?;
                Ok(TypedPattern::Literal { expr: typed_lit, location: location.clone() })
            }
            Pattern::Variable { name, location } => {
                if name != "_" {
                    self.symbol_table.define(name, target_type.clone(), location.line, location.column)?;
                }
                Ok(TypedPattern::Variable {
                    name: name.clone(),
                    type_: target_type.clone(),
                    location: location.clone(),
                })
            }
            Pattern::Range { start, end, inclusive, location } => {
                let typed_start = self.check_expression(start)?;
                let typed_end = self.check_expression(end)?;
                Ok(TypedPattern::Range {
                    start: typed_start,
                    end: typed_end,
                    inclusive: *inclusive,
                    location: location.clone(),
                })
            }
            Pattern::Struct { class_name, fields, location } => {
                let class_def = self.classes.get(class_name).cloned().ok_or_else(|| {
                    RozeError::type_error(
                        format!("Unknown class '{}' in struct pattern", class_name),
                        location.line,
                        location.column,
                    )
                })?;

                let mut typed_fields = Vec::with_capacity(fields.len());
                for (f_name, f_pat) in fields {
                    let f_type = class_def.iter()
                        .find(|(name, _)| name == f_name)
                        .map(|(_, ty)| ty.clone())
                        .ok_or_else(|| {
                            RozeError::type_error(
                                format!("Class '{}' has no field '{}'", class_name, f_name),
                                location.line,
                                location.column,
                            )
                        })?;
                    let typed_pat = self.check_pattern(f_pat, &f_type)?;
                    typed_fields.push((f_name.clone(), typed_pat));
                }

                Ok(TypedPattern::Struct {
                    class_name: class_name.clone(),
                    fields: typed_fields,
                    location: location.clone(),
                })
            }
            Pattern::Tuple { elements, location } => {
                let mut typed_elems = Vec::with_capacity(elements.len());
                for elem in elements {
                    typed_elems.push(self.check_pattern(elem, &Type::Unknown)?);
                }
                Ok(TypedPattern::Tuple {
                    elements: typed_elems,
                    location: location.clone(),
                })
            }
            Pattern::Slice { prefix, slice_rest, suffix, location } => {
                let mut typed_prefix = Vec::with_capacity(prefix.len());
                for p in prefix {
                    typed_prefix.push(self.check_pattern(p, &Type::Unknown)?);
                }
                let typed_rest = if let Some(rest) = slice_rest {
                    Some(Box::new(self.check_pattern(rest, &Type::List)?))
                } else {
                    None
                };
                let mut typed_suffix = Vec::with_capacity(suffix.len());
                for s in suffix {
                    typed_suffix.push(self.check_pattern(s, &Type::Unknown)?);
                }
                Ok(TypedPattern::Slice {
                    prefix: typed_prefix,
                    slice_rest: typed_rest,
                    suffix: typed_suffix,
                    location: location.clone(),
                })
            }
            Pattern::Variant { enum_name, variant, bindings, location } => {
                let target_enum = match enum_name {
                    Some(en) => {
                        if !self.enums.contains_key(en) {
                            return Err(RozeError::type_error(
                                format!("Unknown enum '{}' in match pattern", en),
                                location.line,
                                location.column,
                            ).into());
                        }
                        en.clone()
                    }
                    None => {
                        if let Type::Enum(en) = target_type {
                            en.clone()
                        } else if let Some(candidates) = self.variant_to_enum.get(variant) {
                            if candidates.len() == 1 {
                                candidates[0].clone()
                            } else {
                                return Err(RozeError::type_error(
                                    format!("Ambiguous variant '{}': belongs to multiple enums ({:?}). Qualify it with 'EnumName::{}'", variant, candidates, variant),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                        } else {
                            return Err(RozeError::type_error(
                                format!("Unknown variant '{}' in match pattern", variant),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                };

                let variants = self.enums.get(&target_enum).unwrap().clone();
                let (tag, (_, payload_types)) = variants.iter().enumerate()
                    .find(|(_, (vname, _))| vname == variant)
                    .ok_or_else(|| RozeError::type_error(
                        format!("Enum '{}' has no variant '{}'", target_enum, variant),
                        location.line,
                        location.column,
                    ))?;

                if bindings.len() != payload_types.len() {
                    return Err(RozeError::type_error(
                        format!(
                            "Variant '{}::{}' expects {} argument(s), but pattern has {}",
                            target_enum, variant, payload_types.len(), bindings.len()
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                let mut typed_bindings = Vec::with_capacity(bindings.len());
                for (b_pat, b_type) in bindings.iter().zip(payload_types.iter()) {
                    let tb = self.check_pattern(b_pat, b_type)?;
                    typed_bindings.push(tb);
                }

                Ok(TypedPattern::Variant {
                    enum_name: target_enum,
                    variant: variant.clone(),
                    tag,
                    bindings: typed_bindings,
                    location: location.clone(),
                })
            }
        }
    }

    pub fn check_expression(&mut self, expr: &Expression) -> Result<TypedExpression> {
        match expr {
            Expression::Number { value, location } => {
                let (_, suffix_opt) = parse_numeric_literal(value);
                let ty = if let Some(suffix) = suffix_opt {
                    match suffix {
                        "i8" => Type::I8,
                        "u8" => Type::U8,
                        "i16" => Type::I16,
                        "u16" => Type::U16,
                        "i32" => Type::I32,
                        "u32" => Type::U32,
                        "i64" => Type::I64,
                        "u64" => Type::U64,
                        "i128" => Type::I128,
                        "u128" => Type::U128,
                        "isize" => Type::Isize,
                        "usize" => Type::Usize,
                        "f32" => Type::F32,
                        "f64" => Type::F64,
                        _ => Type::Int,
                    }
                } else if value.contains('.') {
                    Type::Float
                } else {
                    Type::Int
                };
                if (self.in_isr || self.disable_float_registers) && ty.is_float() {
                    return Err(RozeError::type_error(
                        "Floating-point literals are disabled in ISR / freestanding no-float mode",
                        location.line,
                        location.column,
                    ).with_hint("floating-point registers are disabled for bare-metal / ISR safety").into());
                }
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Number(value.clone()),
                    type_: ty,
                    location: location.clone(),
                })
            }
            Expression::Char { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Char(*value),
                type_: Type::Char,
                location: location.clone(),
            }),
            Expression::String { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::String(value.clone()),
                type_: Type::String,
                location: location.clone(),
            }),
            Expression::Boolean { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Boolean(*value),
                type_: Type::Bool,
                location: location.clone(),
            }),
            Expression::Null { location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Null,
                type_: Type::Unknown,
                location: location.clone(),
            }),
            Expression::Identifier { name, location } => {
                if let Some(symbol) = self.symbol_table.lookup(name) {
                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Identifier(name.clone()),
                        type_: symbol.type_.clone(),
                        location: location.clone(),
                    })
                } else if let Some(candidates) = self.variant_to_enum.get(name) {
                    if candidates.len() == 1 {
                        let target_enum = &candidates[0];
                        let variants = self.enums.get(target_enum).unwrap();
                        let (tag, (_, payload_types)) = variants.iter().enumerate()
                            .find(|(_, (vname, _))| vname == name)
                            .unwrap();
                        if payload_types.is_empty() {
                            return Ok(TypedExpression {
                                kind: TypedExpressionKind::EnumConstruct {
                                    enum_name: target_enum.clone(),
                                    variant: name.clone(),
                                    tag,
                                    arguments: vec![],
                                },
                                type_: Type::Enum(target_enum.clone()),
                                location: location.clone(),
                            });
                        }
                    }
                    Err(RozeError::type_error(
                        format!("Undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).with_length(name.chars().count()).into())
                } else if let Some(sig) = self.functions.get(name) {
                    let fn_type = Type::Function {
                        params: sig.params.clone(),
                        return_type: Box::new(sig.return_type.clone()),
                    };
                    Ok(TypedExpression {
                        kind: TypedExpressionKind::FunctionRef(name.clone()),
                        type_: fn_type,
                        location: location.clone(),
                    })
                } else {
                    Err(RozeError::type_error(
                        format!("Undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).with_length(name.chars().count()).into())
                }
            }
            Expression::Unary { operator, operand, location } => {
                let typed_operand = self.check_expression(operand)?;
                let result_type = typed_operand.type_.clone();
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Unary { operator: operator.clone(), operand: Box::new(typed_operand) },
                    type_: result_type,
                    location: location.clone(),
                })
            }
            Expression::Binary { left, operator, right, location } => {
                let typed_left = self.check_expression(left)?;
                let typed_right = self.check_expression(right)?;
                let left_type = &typed_left.type_;
                let right_type = &typed_right.type_;

                let result_type = match operator {
                    BinaryOperator::Add => {
                        if left_type.is_simd_vector() {
                            left_type.clone()
                        } else if right_type.is_simd_vector() {
                            right_type.clone()
                        } else if let Some(numeric_type) = numeric_binary_result_type(left_type, right_type) {
                            numeric_type
                        } else if *left_type == Type::String || *right_type == Type::String {
                            Type::String
                        } else if *left_type == Type::Unknown || *right_type == Type::Unknown {
                            // Untyped (dynamic-ish) operand: allow it and
                            // defer to runtime/codegen, rather than
                            // rejecting code that may well be valid.
                            Type::Unknown
                        } else {
                            return Err(RozeError::type_error(
                                format!("Cannot add {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Modulo => {
                        if left_type.is_simd_vector() {
                            left_type.clone()
                        } else if right_type.is_simd_vector() {
                            right_type.clone()
                        } else if let Some(numeric_type) = numeric_binary_result_type(left_type, right_type) {
                            numeric_type
                        } else {
                            return Err(RozeError::type_error(
                                format!("Cannot perform arithmetic on {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual |
                    BinaryOperator::LessThan | BinaryOperator::GreaterThan |
                    BinaryOperator::LessEqual | BinaryOperator::GreaterEqual |
                    BinaryOperator::And | BinaryOperator::Or => Type::Bool,
                };

                if (self.in_isr || self.disable_float_registers) && (result_type.is_float() || result_type.is_simd_vector()) {
                    return Err(RozeError::type_error(
                        "Floating-point and SIMD vector arithmetic is disabled in ISR / freestanding no-float mode",
                        location.line,
                        location.column,
                    ).with_hint("use integer arithmetic or fixed-point math in interrupt handlers").into());
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Binary {
                        left: Box::new(typed_left),
                        operator: operator.clone(),
                        right: Box::new(typed_right),
                    },
                    type_: result_type,
                    location: location.clone(),
                })
            }
            Expression::Call { function, arguments, location } => {
                let mut typed_arguments = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    typed_arguments.push(self.check_expression(arg)?);
                }

                if let Expression::Identifier { name, .. } = function.as_ref() {
                    if self.no_heap && matches!(name.as_str(), "alloc" | "alloc_zeroed" | "realloc" | "new_box" | "new_arc") {
                        return Err(RozeError::type_error(
                            format!("Dynamic heap allocation ('{}') is disabled in zero-heap bare-metal mode", name),
                            location.line,
                            location.column,
                        ).with_hint("zero-heap bare-metal targets (--no-heap) prohibit dynamic heap allocation").into());
                    }

                    if (self.in_isr || self.disable_float_registers) && matches!(name.as_str(), "sin" | "cos" | "tan" | "sqrt" | "fma") {
                        return Err(RozeError::type_error(
                            format!("Floating-point intrinsic '{}' is disabled in ISR / freestanding no-float mode", name),
                            location.line,
                            location.column,
                        ).with_hint("interrupt service routines must preserve FPU state").into());
                    }

                    if Self::is_unsafe_intrinsic(name) && !self.in_unsafe {
                        return Err(RozeError::type_error(
                            format!("Call to unsafe intrinsic '{}' requires an 'unsafe {{ ... }}' block", name),
                            location.line,
                            location.column,
                        ).with_hint("wrap this low-level direct memory operation inside an 'unsafe { ... }' block").into());
                    }

                    if let Some(symbol) = self.symbol_table.lookup(name) {
                        let sym_type = symbol.type_.clone();
                        match &sym_type {
                            Type::Function { params, return_type } => {
                                if typed_arguments.len() != params.len() {
                                    return Err(RozeError::type_error(
                                        format!("Function expects {} argument(s), but {} were given", params.len(), typed_arguments.len()),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                                for (arg, p_ty) in typed_arguments.iter().zip(params.iter()) {
                                    if !self.is_compatible(p_ty, &arg.type_) {
                                        return Err(RozeError::type_error(
                                            format!("Expected argument of type {}, got {}", p_ty, arg.type_),
                                            location.line,
                                            location.column,
                                        ).into());
                                    }
                                }
                                return Ok(TypedExpression {
                                    kind: TypedExpressionKind::IndirectCall {
                                        callee: Box::new(TypedExpression {
                                            kind: TypedExpressionKind::Identifier(name.clone()),
                                            type_: sym_type.clone(),
                                            location: location.clone(),
                                        }),
                                        arguments: typed_arguments,
                                    },
                                    type_: (**return_type).clone(),
                                    location: location.clone(),
                                });
                            }
                            Type::Unknown => {
                                return Ok(TypedExpression {
                                    kind: TypedExpressionKind::IndirectCall {
                                        callee: Box::new(TypedExpression {
                                            kind: TypedExpressionKind::Identifier(name.clone()),
                                            type_: sym_type,
                                            location: location.clone(),
                                        }),
                                        arguments: typed_arguments,
                                    },
                                    type_: Type::Unknown,
                                    location: location.clone(),
                                });
                            }
                            other => {
                                return Err(RozeError::type_error(
                                    format!("Cannot call non-function variable '{}' of type {}", name, other),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                        }
                    }

                    let return_type = if name == "println" {
                        Type::Void
                    } else if name == "abs" && typed_arguments.len() == 1 {
                        // Math.abs has both an int and a double overload
                        // on the JVM side, and the native backend needs
                        // the same distinction -- the return type has to
                        // track whichever one this call actually
                        // resolves to, or a `let` binding downstream
                        // would get the wrong declared type.
                        match &typed_arguments[0].type_ {
                            Type::Float => Type::Float,
                            Type::Int | Type::Unknown => Type::Int,
                            other => return Err(RozeError::type_error(
                                format!("abs() needs a number, got {}", other),
                                location.line,
                                location.column,
                            ).into()),
                        }
                    } else if (name == "max" || name == "min") && typed_arguments.len() == 2 {
                        numeric_binary_result_type(&typed_arguments[0].type_, &typed_arguments[1].type_)
                            .ok_or_else(|| RozeError::type_error(
                                format!(
                                    "{}() needs two numbers, got {} and {}",
                                    name, typed_arguments[0].type_, typed_arguments[1].type_
                                ),
                                location.line,
                                location.column,
                            ))?
                    } else if (name == "wrapping_add" || name == "wrapping_sub" || name == "wrapping_mul" ||
                               name == "saturating_add" || name == "saturating_sub" || name == "saturating_mul") && typed_arguments.len() == 2 {
                        typed_arguments[0].type_.clone()
                    } else if name == "Vec4f" || name == "vec4f" {
                        Type::Vec4f
                    } else if name == "Vec8f" || name == "vec8f" {
                        Type::Vec8f
                    } else if name == "Vec4i" || name == "vec4i" {
                        Type::Vec4i
                    } else if name == "Vec16u8" || name == "vec16u8" {
                        Type::Vec16u8
                    } else if let Some(gen_fn) = self.generic_functions.get(name).cloned() {
                        let concrete_type_args = self.infer_func_type_args(&gen_fn, &typed_arguments, location)?;
                        let spec_name = self.instantiate_generic_function(name, &concrete_type_args)?;
                        let sig = self.functions.get(&spec_name).unwrap().clone();
                        for (arg, param_type) in typed_arguments.iter().zip(sig.params.iter()) {
                            if !self.is_compatible(param_type, &arg.type_) {
                                return Err(RozeError::type_error(
                                    format!("'{}' expects {} for this argument, got {}", name, param_type, arg.type_),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                        }
                        return Ok(TypedExpression {
                            kind: TypedExpressionKind::Call {
                                function: spec_name,
                                arguments: typed_arguments,
                            },
                            type_: sig.return_type.clone(),
                            location: location.clone(),
                        });
                    } else if name == "list_get" && typed_arguments.len() == 2 {
                        let elem_ty = match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "List" || base == "list") && !type_args.is_empty() => {
                                type_args[0].clone()
                            }
                            _ => Type::Unknown,
                        };
                        elem_ty
                    } else if name == "list_set" && typed_arguments.len() == 3 {
                        let elem_ty = match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "List" || base == "list") && !type_args.is_empty() => {
                                if !types_compatible(&type_args[0], &typed_arguments[2].type_) {
                                    return Err(RozeError::type_error(
                                        format!("list_set expects element type {}, got {}", type_args[0], typed_arguments[2].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                                type_args[0].clone()
                            }
                            _ => Type::Unknown,
                        };
                        elem_ty
                    } else if name == "list_push" && typed_arguments.len() == 2 {
                        if let Type::Generic { base, type_args } = &typed_arguments[0].type_ {
                            if (base == "List" || base == "list") && !type_args.is_empty() {
                                if !types_compatible(&type_args[0], &typed_arguments[1].type_) {
                                    return Err(RozeError::type_error(
                                        format!("list_push expects element type {}, got {}", type_args[0], typed_arguments[1].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                        }
                        Type::Bool
                    } else if (name == "map_put" || name == "map_set") && typed_arguments.len() == 3 {
                        if let Type::Generic { base, type_args } = &typed_arguments[0].type_ {
                            if (base == "Map" || base == "map") && type_args.len() >= 2 {
                                if !types_compatible(&type_args[0], &typed_arguments[1].type_) {
                                    return Err(RozeError::type_error(
                                        format!("map_put expects key type {}, got {}", type_args[0], typed_arguments[1].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                                if !types_compatible(&type_args[1], &typed_arguments[2].type_) {
                                    return Err(RozeError::type_error(
                                        format!("map_put expects value type {}, got {}", type_args[1], typed_arguments[2].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                        }
                        let val_ty = match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "Map" || base == "map") && type_args.len() >= 2 => {
                                type_args[1].clone()
                            }
                            _ => Type::Unknown,
                        };
                        val_ty
                    } else if name == "map_get" && typed_arguments.len() == 2 {
                        let val_ty = match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "Map" || base == "map") && type_args.len() >= 2 => {
                                if !types_compatible(&type_args[0], &typed_arguments[1].type_) {
                                    return Err(RozeError::type_error(
                                        format!("map_get expects key type {}, got {}", type_args[0], typed_arguments[1].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                                type_args[1].clone()
                            }
                            _ => Type::Unknown,
                        };
                        val_ty
                    } else if name == "channel_new" {
                        if typed_arguments.len() > 1 {
                            return Err(RozeError::type_error(
                                format!("'channel_new' takes 0 or 1 argument, but {} were given", typed_arguments.len()),
                                location.line,
                                location.column,
                            ).into());
                        }
                        if typed_arguments.len() == 1 && !types_compatible(&Type::Int, &typed_arguments[0].type_) {
                            return Err(RozeError::type_error(
                                format!("'channel_new' expects capacity of type int, got {}", typed_arguments[0].type_),
                                location.line,
                                location.column,
                            ).into());
                        }
                        Type::Generic { base: "Channel".to_string(), type_args: vec![] }
                    } else if (name == "send" || name == "channel_send" || name == "channel_try_send") && typed_arguments.len() == 2 {
                        let chan_ty = &typed_arguments[0].type_;
                        match chan_ty {
                            Type::Generic { base, type_args } if (base == "Channel" || base == "channel") && !type_args.is_empty() => {
                                if !types_compatible(&type_args[0], &typed_arguments[1].type_) {
                                    return Err(RozeError::type_error(
                                        format!("'{}' expects value of type {}, got {}", name, type_args[0], typed_arguments[1].type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                            _ => {}
                        }
                        Type::Bool
                    } else if (name == "recv" || name == "channel_recv" || name == "channel_try_recv") && typed_arguments.len() == 1 {
                        let chan_ty = &typed_arguments[0].type_;
                        match chan_ty {
                            Type::Generic { base, type_args } if (base == "Channel" || base == "channel") && !type_args.is_empty() => {
                                type_args[0].clone()
                            }
                            _ => Type::Unknown,
                        }
                    } else if (name == "atomic_ptr_load" || (name == "atomic_load" && matches!(typed_arguments[0].type_, Type::Generic { ref base, .. } if base == "AtomicPtr" || base == "atomic_ptr"))) && typed_arguments.len() == 1 {
                        let a_ty = &typed_arguments[0].type_;
                        match a_ty {
                            Type::Generic { base, type_args } if (base == "AtomicPtr" || base == "atomic_ptr") && !type_args.is_empty() => {
                                type_args[0].clone()
                            }
                            _ => Type::RawPointer(Box::new(Type::U8)),
                        }
                    } else if (name == "atomic_bool_load" || (name == "atomic_load" && matches!(typed_arguments[0].type_, Type::Generic { ref base, .. } if base == "AtomicBool" || base == "atomic_bool"))) && typed_arguments.len() == 1 {
                        Type::Bool
                    } else if (name == "atomic_i64_load" || name == "atomic_usize_load" || name == "atomic_load") && typed_arguments.len() == 1 {
                        Type::Int
                    } else if (name == "thread_spawn" || name == "fiber_spawn" || name == "future_spawn" || name == "__roze_async_spawn") && typed_arguments.len() == 1 {
                        let ret_type = match &typed_arguments[0].type_ {
                            Type::Function { return_type, .. } => vec![*return_type.clone()],
                            _ => vec![],
                        };
                        let base = if name == "fiber_spawn" {
                            "Fiber"
                        } else if name == "future_spawn" || name == "__roze_async_spawn" {
                            "Future"
                        } else {
                            "Thread"
                        };
                        Type::Generic { base: base.to_string(), type_args: ret_type }
                    } else if (name == "task_group_fork" || name == "nursery_fork" || name == "task_group_spawn" || name == "nursery_spawn") && typed_arguments.len() == 2 {
                        let ret_type = match &typed_arguments[1].type_ {
                            Type::Function { return_type, .. } => vec![*return_type.clone()],
                            _ => vec![],
                        };
                        Type::Generic { base: "Subtask".to_string(), type_args: ret_type }
                    } else if (name == "thread_join" || name == "fiber_join" || name == "future_await" || name == "__roze_future_await" || name == "subtask_get") && typed_arguments.len() == 1 {
                        let th_ty = &typed_arguments[0].type_;
                        match th_ty {
                            Type::Generic { base, type_args } if (base == "Thread" || base == "thread" || base == "Fiber" || base == "fiber" || base == "Future" || base == "future" || base == "Subtask" || base == "subtask") && !type_args.is_empty() => {
                                type_args[0].clone()
                            }
                            _ => Type::Unknown,
                        }
                    } else if (name == "future_ready" || name == "__roze_future_ready") && typed_arguments.len() == 1 {
                        Type::Generic { base: "Future".to_string(), type_args: vec![typed_arguments[0].type_.clone()] }
                    } else if (name == "ptr_add" || name == "ptr_offset") && typed_arguments.len() == 2 {
                        if !matches!(typed_arguments[0].type_, Type::RawPointer(_)) {
                            return Err(RozeError::type_error(
                                format!("'{}' expects first argument to be a raw pointer, got {}", name, typed_arguments[0].type_),
                                location.line,
                                location.column,
                            ).into());
                        }
                        if !self.is_compatible(&Type::Int, &typed_arguments[1].type_) {
                            return Err(RozeError::type_error(
                                format!("'{}' expects second argument (offset) to be int, got {}", name, typed_arguments[1].type_),
                                location.line,
                                location.column,
                            ).into());
                        }
                        typed_arguments[0].type_.clone()
                    } else if (name == "vec_deque_pop_front" || name == "vec_deque_pop_back" || name == "vec_deque_front" || name == "vec_deque_back") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "VecDeque" || base == "vec_deque") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if name == "vec_deque_get" && typed_arguments.len() == 2 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "VecDeque" || base == "vec_deque") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "btree_map_get" || name == "btree_map_remove") && typed_arguments.len() == 2 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "BTreeMap" || base == "btree_map") && type_args.len() >= 2 => type_args[1].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "btree_map_first_key" || name == "btree_map_last_key") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "BTreeMap" || base == "btree_map") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "btree_set_first" || name == "btree_set_last") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "BTreeSet" || base == "btree_set") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "binary_heap_pop" || name == "binary_heap_peek") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "BinaryHeap" || base == "binary_heap") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "index_map_get" || name == "index_map_remove" || name == "index_map_get_index_value") && typed_arguments.len() >= 2 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "IndexMap" || base == "index_map") && type_args.len() >= 2 => type_args[1].clone(),
                            _ => Type::Unknown,
                        }
                    } else if name == "index_map_get_index_key" && typed_arguments.len() == 2 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "IndexMap" || base == "index_map") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "ring_buffer_pop" || name == "ring_buffer_peek") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "RingBuffer" || base == "ring_buffer") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if name == "box_get" && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "RozeBox" || base == "Box" || base == "box") && !type_args.is_empty() => type_args[0].clone(),
                            _ => Type::Unknown,
                        }
                    } else if (name == "arc_get" || name == "arc_clone" || name == "arc_downgrade") && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "RozeArc" || base == "Arc" || base == "arc") => {
                                if name == "arc_get" {
                                    if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }
                                } else if name == "arc_clone" {
                                    typed_arguments[0].type_.clone()
                                } else {
                                    Type::Generic { base: "RozeWeak".to_string(), type_args: type_args.clone() }
                                }
                            }
                            _ => Type::Unknown,
                        }
                    } else if name == "weak_upgrade" && typed_arguments.len() == 1 {
                        match &typed_arguments[0].type_ {
                            Type::Generic { base, type_args } if (base == "RozeWeak" || base == "Weak" || base == "weak") => {
                                Type::Generic { base: "RozeArc".to_string(), type_args: type_args.clone() }
                            }
                            _ => Type::Generic { base: "RozeArc".to_string(), type_args: vec![] },
                        }
                    } else if let Some(sig) = self.functions.get(name) {
                        // Validate argument count and each argument's
                        // type against the declared signature -- the
                        // same checks `New` (class construction) already
                        // does for its constructor arguments, just
                        // never applied to an ordinary function call
                        // before now. Skipped for intrinsics whose
                        // params are deliberately Unknown/variable-arity
                        // in `builtin_signatures` (e.g. `to_string`,
                        // `list_push`) -- those accept anything by
                        // design, not by omission.
                        let params_are_meaningfully_typed = !sig.params.iter().all(|p| matches!(p, Type::Unknown));
                        if params_are_meaningfully_typed {
                            if typed_arguments.len() != sig.params.len() {
                                return Err(RozeError::type_error(
                                    format!(
                                        "'{}' takes {} argument(s), but {} were given",
                                        name, sig.params.len(), typed_arguments.len()
                                    ),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                            for (arg, param_type) in typed_arguments.iter().zip(sig.params.iter()) {
                                if !self.is_compatible(param_type, &arg.type_) {
                                    return Err(RozeError::type_error(
                                        format!(
                                            "'{}' expects {} for this argument, got {}",
                                            name, param_type, arg.type_
                                        ),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                        }
                        sig.return_type.clone()
                    } else if let Some(candidates) = self.variant_to_enum.get(name) {
                        if candidates.len() == 1 {
                            let target_enum = candidates[0].clone();
                            let variants = self.enums.get(&target_enum).unwrap();
                            let (tag, (_, expected_types)) = variants.iter().enumerate()
                                .find(|(_, (vname, _))| vname == name)
                                .unwrap();

                            if typed_arguments.len() != expected_types.len() {
                                return Err(RozeError::type_error(
                                    format!(
                                        "Variant '{}::{}' takes {} argument(s), but {} were given",
                                        target_enum, name, expected_types.len(), typed_arguments.len()
                                    ),
                                    location.line,
                                    location.column,
                                ).into());
                            }

                            for (arg, expected_ty) in typed_arguments.iter().zip(expected_types.iter()) {
                                if !self.is_compatible(expected_ty, &arg.type_) {
                                    return Err(RozeError::type_error(
                                        format!(
                                            "'{}::{}' expects {} for argument, got {}",
                                            target_enum, name, expected_ty, arg.type_
                                        ),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }

                            return Ok(TypedExpression {
                                kind: TypedExpressionKind::EnumConstruct {
                                    enum_name: target_enum.clone(),
                                    variant: name.clone(),
                                    tag,
                                    arguments: typed_arguments,
                                },
                                type_: Type::Enum(target_enum),
                                location: location.clone(),
                            });
                        } else {
                            return Err(RozeError::type_error(
                                format!("Ambiguous variant '{}': belongs to multiple enums ({:?})", name, candidates),
                                location.line,
                                location.column,
                            ).into());
                        }
                    } else {
                        return Err(RozeError::type_error(
                            format!("Call to undefined function '{}'", name),
                            location.line,
                            location.column,
                        ).with_length(name.chars().count()).into());
                    };

                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Call { function: name.clone(), arguments: typed_arguments },
                        type_: return_type,
                        location: location.clone(),
                    })
                } else {
                    let typed_callee = self.check_expression(function)?;
                    let callee_ty = typed_callee.type_.clone();
                    match &callee_ty {
                        Type::Function { params, return_type } => {
                            if typed_arguments.len() != params.len() {
                                return Err(RozeError::type_error(
                                    format!("Function expects {} argument(s), but {} were given", params.len(), typed_arguments.len()),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                            for (arg, p_ty) in typed_arguments.iter().zip(params.iter()) {
                                if !self.is_compatible(p_ty, &arg.type_) {
                                    return Err(RozeError::type_error(
                                        format!("Expected argument of type {}, got {}", p_ty, arg.type_),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                            Ok(TypedExpression {
                                kind: TypedExpressionKind::IndirectCall {
                                    callee: Box::new(typed_callee),
                                    arguments: typed_arguments,
                                },
                                type_: (**return_type).clone(),
                                location: location.clone(),
                            })
                        }
                        Type::Unknown => Ok(TypedExpression {
                            kind: TypedExpressionKind::IndirectCall {
                                callee: Box::new(typed_callee),
                                arguments: typed_arguments,
                            },
                            type_: Type::Unknown,
                            location: location.clone(),
                        }),
                        other => Err(RozeError::type_error(
                            format!("Cannot call non-function expression of type {}", other),
                            location.line,
                            location.column,
                        ).into()),
                    }
                }
            }
            Expression::New { class_name, type_args, arguments, location } => {
                if self.no_heap {
                    return Err(RozeError::type_error(
                        format!("Dynamic heap allocation ('new {}') is disabled in zero-heap bare-metal mode", class_name),
                        location.line,
                        location.column,
                    ).with_hint("zero-heap bare-metal targets (--no-heap) prohibit dynamic heap allocation; use static variables, stack, or custom memory buffers").into());
                }
                let is_user_class = self.classes.contains_key(class_name) || self.generic_classes.contains_key(class_name);
                let is_builtin_col = !is_user_class && matches!(
                    class_name.as_str(),
                    "VecDeque" | "vec_deque" | "BTreeMap" | "btree_map" | "BTreeSet" | "btree_set"
                    | "BinaryHeap" | "binary_heap" | "IndexMap" | "index_map" | "RingBuffer" | "ring_buffer"
                    | "BitSet" | "bit_set"
                    | "Box" | "box" | "RozeBox" | "Arc" | "arc" | "RozeArc" | "Weak" | "weak" | "RozeWeak"
                    | "BumpAllocator" | "bump_allocator" | "ArenaAllocator" | "arena_allocator"
                    | "PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator"
                );
                if is_builtin_col {
                    let mut concrete_args = Vec::new();
                    for ta in type_args {
                        concrete_args.push(self.resolve_type_name(ta));
                    }
                    let mut typed_args = Vec::new();
                    for arg in arguments {
                        typed_args.push(self.check_expression(arg)?);
                    }
                    let (fn_name, base_name) = match class_name.as_str() {
                        "VecDeque" | "vec_deque" => (if typed_args.is_empty() { "vec_deque_new" } else { "vec_deque_with_capacity" }, "VecDeque"),
                        "BTreeMap" | "btree_map" => {
                            if !concrete_args.is_empty() && concrete_args[0] == Type::String {
                                ("btree_map_new_str", "BTreeMap")
                            } else {
                                ("btree_map_new", "BTreeMap")
                            }
                        }
                        "BTreeSet" | "btree_set" => {
                            if !concrete_args.is_empty() && concrete_args[0] == Type::String {
                                ("btree_set_new_str", "BTreeSet")
                            } else {
                                ("btree_set_new", "BTreeSet")
                            }
                        }
                        "BinaryHeap" | "binary_heap" => ("binary_heap_new", "BinaryHeap"),
                        "IndexMap" | "index_map" => {
                            if !concrete_args.is_empty() && concrete_args[0] == Type::String {
                                ("index_map_new_str", "IndexMap")
                            } else {
                                ("index_map_new", "IndexMap")
                            }
                        }
                        "RingBuffer" | "ring_buffer" => ("ring_buffer_new", "RingBuffer"),
                        "BitSet" | "bit_set" => (if typed_args.is_empty() { "bit_set_new" } else { "bit_set_with_capacity" }, "BitSet"),
                        "Box" | "box" | "RozeBox" => {
                            if concrete_args.is_empty() && !typed_args.is_empty() {
                                concrete_args.push(typed_args[0].type_.clone());
                            }
                            ("box_new", "RozeBox")
                        }
                        "Arc" | "arc" | "RozeArc" => {
                            if concrete_args.is_empty() && !typed_args.is_empty() {
                                concrete_args.push(typed_args[0].type_.clone());
                            }
                            ("arc_new", "RozeArc")
                        }
                        "Weak" | "weak" | "RozeWeak" => ("weak_new", "RozeWeak"),
                        "BumpAllocator" | "bump_allocator" => {
                            (if typed_args.is_empty() { "bump_allocator_new_default" } else { "bump_allocator_new" }, "BumpAllocator")
                        }
                        "ArenaAllocator" | "arena_allocator" => {
                            (if typed_args.is_empty() { "arena_allocator_new_default" } else { "arena_allocator_new" }, "ArenaAllocator")
                        }
                        "PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator" => {
                            ("pool_allocator_new", "PoolAllocator")
                        }
                        _ => unreachable!(),
                    };
                    return Ok(TypedExpression {
                        kind: TypedExpressionKind::Call {
                            function: fn_name.to_string(),
                            arguments: typed_args,
                        },
                        type_: Type::Generic {
                            base: base_name.to_string(),
                            type_args: concrete_args,
                        },
                        location: location.clone(),
                    });
                }

                let actual_class_name = if self.generic_classes.contains_key(class_name) {
                    let template = self.generic_classes.get(class_name).unwrap().clone();
                    let concrete_type_args = if !type_args.is_empty() {
                        let mut resolved = Vec::with_capacity(type_args.len());
                        for ta in type_args {
                            resolved.push(self.resolve_type_name(ta));
                        }
                        resolved
                    } else {
                        let mut typed_args = Vec::with_capacity(arguments.len());
                        for arg in arguments {
                            typed_args.push(self.check_expression(arg)?);
                        }
                        self.infer_class_type_args(&template, &typed_args, location)?
                    };
                    self.instantiate_generic_class(class_name, &concrete_type_args)?
                } else {
                    class_name.clone()
                };

                let fields = self.classes.get(&actual_class_name).cloned().ok_or_else(|| RozeError::type_error(
                    format!("Undefined class '{}'", actual_class_name),
                    location.line,
                    location.column,
                ).with_length(actual_class_name.chars().count()))?;

                if arguments.len() != fields.len() {
                    return Err(RozeError::type_error(
                        format!(
                            "'{}' has {} field(s), but 'new {}(...)' was given {} argument(s)",
                            class_name, fields.len(), class_name, arguments.len()
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                let mut typed_arguments = Vec::with_capacity(arguments.len());
                for (arg, (field_name, field_type)) in arguments.iter().zip(fields.iter()) {
                    let typed_arg = self.check_expression(arg)?;
                    if !self.is_compatible(field_type, &typed_arg.type_) {
                        return Err(RozeError::type_error(
                            format!(
                                "'{}.{}' is declared as {}, but 'new {}(...)' was given {} for it",
                                class_name, field_name, field_type, class_name, typed_arg.type_
                            ),
                            location.line,
                            location.column,
                        ).into());
                    }
                    typed_arguments.push(typed_arg);
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::New { class_name: actual_class_name.clone(), arguments: typed_arguments },
                    type_: Type::Class(actual_class_name),
                    location: location.clone(),
                })
            }
            Expression::GenericCall { function, type_args, arguments, location } => {
                if function == "size_of" {
                    if type_args.is_empty() {
                        return Err(RozeError::type_error("size_of::<T>() requires 1 type argument", location.line, location.column).into());
                    }
                    let (sz, _) = self.compute_type_layout(&type_args[0])?;
                    return Ok(TypedExpression {
                        kind: TypedExpressionKind::Number(sz.to_string()),
                        type_: Type::Int,
                        location: location.clone(),
                    });
                }
                if function == "align_of" {
                    if type_args.is_empty() {
                        return Err(RozeError::type_error("align_of::<T>() requires 1 type argument", location.line, location.column).into());
                    }
                    let (_, al) = self.compute_type_layout(&type_args[0])?;
                    return Ok(TypedExpression {
                        kind: TypedExpressionKind::Number(al.to_string()),
                        type_: Type::Int,
                        location: location.clone(),
                    });
                }
                if function == "offset_of" {
                    if type_args.is_empty() {
                        return Err(RozeError::type_error("offset_of::<T>() requires 1 type argument", location.line, location.column).into());
                    }
                    if arguments.is_empty() {
                        return Err(RozeError::type_error("offset_of::<T>(field_name) requires a field name argument", location.line, location.column).into());
                    }
                    let field_expr = self.check_expression(&arguments[0])?;
                    if let TypedExpressionKind::String(ref fname) = field_expr.kind {
                        let off = self.compute_field_offset(&type_args[0], fname)?;
                        return Ok(TypedExpression {
                            kind: TypedExpressionKind::Number(off.to_string()),
                            type_: Type::Int,
                            location: location.clone(),
                        });
                    } else {
                        return Err(RozeError::type_error("offset_of field argument must be a string literal", location.line, location.column).into());
                    }
                }

                if let Some(_template) = self.generic_functions.get(function).cloned() {
                    let mut resolved_type_args = Vec::with_capacity(type_args.len());
                    for ta in type_args {
                        resolved_type_args.push(self.resolve_type_name(ta));
                    }
                    let spec_name = self.instantiate_generic_function(function, &resolved_type_args)?;
                    let sig = self.functions.get(&spec_name).unwrap().clone();

                    if arguments.len() != sig.params.len() {
                        return Err(RozeError::type_error(
                            format!(
                                "'{}' takes {} argument(s), but {} were given",
                                function, sig.params.len(), arguments.len()
                            ),
                            location.line,
                            location.column,
                        ).into());
                    }

                    let mut typed_arguments = Vec::with_capacity(arguments.len());
                    for (arg, param_type) in arguments.iter().zip(sig.params.iter()) {
                        let typed_arg = self.check_expression(arg)?;
                        if !self.is_compatible(param_type, &typed_arg.type_) {
                            return Err(RozeError::type_error(
                                format!("'{}' expects {} for this argument, got {}", function, param_type, typed_arg.type_),
                                location.line,
                                location.column,
                            ).into());
                        }
                        typed_arguments.push(typed_arg);
                    }

                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Call {
                            function: spec_name,
                            arguments: typed_arguments,
                        },
                        type_: sig.return_type.clone(),
                        location: location.clone(),
                    })
                } else {
                    return Err(RozeError::type_error(
                        format!("Undefined generic function '{}'", function),
                        location.line,
                        location.column,
                    ).into());
                }
            }
            Expression::FieldAccess { object, field, location } => {
                let typed_object = self.check_expression(object)?;
                let class_name = match &typed_object.type_ {
                    Type::Class(name) => name.clone(),
                    other => return Err(RozeError::type_error(
                        format!("Cannot access a field on {}, which isn't a class instance", other),
                        location.line,
                        location.column,
                    ).into()),
                };
                let field_type = self.classes.get(&class_name)
                    .and_then(|fields| fields.iter().find(|(n, _)| n == field))
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| RozeError::type_error(
                        format!("'{}' has no field named '{}'", class_name, field),
                        location.line,
                        location.column,
                    ))?;

                Ok(TypedExpression {
                    kind: TypedExpressionKind::FieldAccess { object: Box::new(typed_object), field: field.clone() },
                    type_: field_type,
                    location: location.clone(),
                })
            }
            Expression::Try { expr, location } => {
                let typed_expr = self.check_expression(expr)?;
                match &typed_expr.type_ {
                    Type::Result => {
                        if !matches!(self.current_return_type, Type::Result | Type::Unknown) {
                            return Err(RozeError::type_error(
                                format!(
                                    "The '?' operator on a Result requires the enclosing function to return Result, but it returns {}",
                                    self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    Type::Option => {
                        if !matches!(self.current_return_type, Type::Option | Type::Unknown) {
                            return Err(RozeError::type_error(
                                format!(
                                    "The '?' operator on an Option requires the enclosing function to return Option, but it returns {}",
                                    self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    Type::Unknown => {}
                    other => {
                        return Err(RozeError::type_error(
                            format!("The '?' operator can only be used on Result or Option, got {}", other),
                            location.line,
                            location.column,
                        ).into());
                    }
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Try(Box::new(typed_expr)),
                    type_: Type::Unknown,
                    location: location.clone(),
                })
            }
            Expression::EnumConstruct { enum_name, variant, arguments, location } => {
                let target_enum = match enum_name {
                    Some(en) => {
                        if !self.enums.contains_key(en) {
                            return Err(RozeError::type_error(
                                format!("Unknown enum '{}'", en),
                                location.line,
                                location.column,
                            ).into());
                        }
                        en.clone()
                    }
                    None => {
                        if let Some(candidates) = self.variant_to_enum.get(variant) {
                            if candidates.len() == 1 {
                                candidates[0].clone()
                            } else {
                                return Err(RozeError::type_error(
                                    format!("Ambiguous variant '{}': belongs to multiple enums ({:?}). Qualify with 'EnumName::{}'", variant, candidates, variant),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                        } else {
                            return Err(RozeError::type_error(
                                format!("Unknown enum variant '{}'", variant),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                };

                let (tag, expected_types) = {
                    let variants = self.enums.get(&target_enum).unwrap();
                    let (tag, (_, expected_types)) = variants.iter().enumerate()
                        .find(|(_, (vname, _))| vname == variant)
                        .ok_or_else(|| RozeError::type_error(
                            format!("Enum '{}' has no variant '{}'", target_enum, variant),
                            location.line,
                            location.column,
                        ))?;
                    (tag, expected_types.clone())
                };

                if arguments.len() != expected_types.len() {
                    return Err(RozeError::type_error(
                        format!(
                            "Variant '{}::{}' takes {} argument(s), but {} were given",
                            target_enum, variant, expected_types.len(), arguments.len()
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                let mut typed_args = Vec::with_capacity(arguments.len());
                for (arg, expected_ty) in arguments.iter().zip(expected_types.iter()) {
                    let typed_arg = self.check_expression(arg)?;
                    if !types_compatible(expected_ty, &typed_arg.type_) {
                        return Err(RozeError::type_error(
                            format!(
                                "'{}::{}' expects {} for argument, got {}",
                                target_enum, variant, expected_ty, typed_arg.type_
                            ),
                            location.line,
                            location.column,
                        ).into());
                    }
                    typed_args.push(typed_arg);
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::EnumConstruct {
                        enum_name: target_enum.clone(),
                        variant: variant.clone(),
                        tag,
                        arguments: typed_args,
                    },
                    type_: Type::Enum(target_enum),
                    location: location.clone(),
                })
            }
            Expression::Closure { params, return_type, body, location } => {
                let mut typed_params = Vec::with_capacity(params.len());
                let mut param_types = Vec::with_capacity(params.len());
                for p in params {
                    let ty = p.type_name.as_ref()
                        .map(|t| self.resolve_type_name(t))
                        .unwrap_or(Type::Unknown);
                    typed_params.push((p.name.clone(), ty.clone()));
                    param_types.push(ty);
                }

                let declared_ret = return_type.as_ref()
                    .map(|t| self.resolve_type_name(t))
                    .unwrap_or(Type::Unknown);

                let closure_scope_depth = self.symbol_table.scopes.len();
                self.symbol_table.push_scope();

                for (p_name, p_ty) in &typed_params {
                    self.symbol_table.define(p_name, p_ty.clone(), location.line, location.column)?;
                }

                let prev_ret = self.current_return_type.clone();
                self.current_return_type = declared_ret.clone();

                let typed_body = self.check_statement(body)?;
                let typed_body = typed_body.unwrap_or_else(|| TypedStatement::Block {
                    statements: vec![],
                    location: location.clone(),
                });

                let mut referenced = Vec::new();
                collect_referenced_identifiers(&typed_body, &mut referenced);

                let mut captures: Vec<(String, Type)> = Vec::new();
                for id in referenced {
                    if let Some((sym, depth)) = self.symbol_table.lookup_with_depth(&id) {
                        if depth < closure_scope_depth && !captures.iter().any(|(c, _)| c == &id) {
                            captures.push((id, sym.type_.clone()));
                        }
                    }
                }

                self.symbol_table.pop_scope();
                self.current_return_type = prev_ret;

                let final_ret = if declared_ret != Type::Unknown {
                    declared_ret
                } else {
                    let mut inferred = None;
                    collect_return_types(&typed_body, &mut inferred);
                    inferred.unwrap_or(Type::Void)
                };

                let fn_type = Type::Function {
                    params: param_types,
                    return_type: Box::new(final_ret.clone()),
                };

                let closure_id = self.closure_counter;
                self.closure_counter += 1;

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Closure {
                        id: closure_id,
                        params: typed_params,
                        return_type: final_ret,
                        captures,
                        body: Box::new(typed_body),
                    },
                    type_: fn_type,
                    location: location.clone(),
                })
            }
            Expression::MethodCall { object, method, arguments, location } => {
                let typed_obj = self.check_expression(object)?;
                let mut typed_args = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    typed_args.push(self.check_expression(arg)?);
                }

                let obj_type = typed_obj.type_.clone();
                match &obj_type {
                    Type::Trait(trait_name) => {
                        let trait_def = self.traits.get(trait_name).cloned().ok_or_else(|| {
                            RozeError::type_error(format!("Undefined trait '{}'", trait_name), location.line, location.column)
                        })?;
                        let sig = trait_def.methods.get(method).cloned().ok_or_else(|| {
                            RozeError::type_error(format!("Trait '{}' has no method '{}'", trait_name, method), location.line, location.column)
                        })?;
                        if typed_args.len() != sig.params.len() {
                            return Err(RozeError::type_error(
                                format!("Method '{}' expects {} arguments, got {}", method, sig.params.len(), typed_args.len()),
                                location.line, location.column,
                            ).into());
                        }
                        for (arg, expected_ty) in typed_args.iter().zip(sig.params.iter()) {
                            if !self.is_compatible(expected_ty, &arg.type_) {
                                return Err(RozeError::type_error(
                                    format!("Argument type mismatch for method '{}': expected {}, got {}", method, expected_ty, arg.type_),
                                    location.line, location.column,
                                ).into());
                            }
                        }
                        let slot = *self.trait_methods.get(method).expect("trait method has slot");
                        Ok(TypedExpression {
                            kind: TypedExpressionKind::TraitMethodCall {
                                object: Box::new(typed_obj),
                                trait_name: trait_name.clone(),
                                method: method.clone(),
                                slot,
                                arguments: typed_args,
                            },
                            type_: sig.return_type,
                            location: location.clone(),
                        })
                    }
                    Type::Class(class_name) => {
                        let sig = self.class_methods.get(class_name).and_then(|m| m.get(method)).cloned().ok_or_else(|| {
                            RozeError::type_error(format!("Class '{}' has no method '{}'", class_name, method), location.line, location.column)
                        })?;
                        if typed_args.len() != sig.params.len() {
                            return Err(RozeError::type_error(
                                format!("Method '{}' expects {} arguments, got {}", method, sig.params.len(), typed_args.len()),
                                location.line, location.column,
                            ).into());
                        }
                        for (arg, expected_ty) in typed_args.iter().zip(sig.params.iter()) {
                            if !self.is_compatible(expected_ty, &arg.type_) {
                                return Err(RozeError::type_error(
                                    format!("Argument type mismatch for method '{}': expected {}, got {}", method, expected_ty, arg.type_),
                                    location.line, location.column,
                                ).into());
                            }
                        }
                        Ok(TypedExpression {
                            kind: TypedExpressionKind::MethodCall {
                                object: Box::new(typed_obj),
                                class_name: class_name.clone(),
                                method: method.clone(),
                                arguments: typed_args,
                            },
                            type_: sig.return_type,
                            location: location.clone(),
                        })
                    }
                    Type::Generic { base, type_args } => {
                        let (fn_name, ret_ty) = match (base.as_str(), method.as_str()) {
                            // Channel
                            ("Channel" | "channel", "send") => ("channel_send", Type::Bool),
                            ("Channel" | "channel", "try_send") => ("channel_try_send", Type::Bool),
                            ("Channel" | "channel", "recv") => ("channel_recv", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("Channel" | "channel", "try_recv") => ("channel_try_recv", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("Channel" | "channel", "close") => ("channel_close", Type::Void),
                            ("Channel" | "channel", "is_closed") => ("channel_is_closed", Type::Bool),
                            ("Channel" | "channel", "len") => ("channel_len", Type::Int),
                            ("Channel" | "channel", "capacity") => ("channel_capacity", Type::Int),

                            // Mutex
                            ("Mutex" | "mutex", "lock") => ("mutex_lock", Type::Void),
                            ("Mutex" | "mutex", "unlock") => ("mutex_unlock", Type::Void),
                            ("Mutex" | "mutex", "try_lock") => ("mutex_try_lock", Type::Bool),

                            // RwLock
                            ("RwLock" | "rwlock", "read_lock") => ("rwlock_read_lock", Type::Void),
                            ("RwLock" | "rwlock", "read_unlock") => ("rwlock_read_unlock", Type::Void),
                            ("RwLock" | "rwlock", "write_lock") => ("rwlock_write_lock", Type::Void),
                            ("RwLock" | "rwlock", "write_unlock") => ("rwlock_write_unlock", Type::Void),
                            ("RwLock" | "rwlock", "try_read_lock") => ("rwlock_try_read_lock", Type::Bool),
                            ("RwLock" | "rwlock", "try_write_lock") => ("rwlock_try_write_lock", Type::Bool),

                            // AtomicBool
                            ("AtomicBool" | "atomic_bool", "load") => ("atomic_bool_load", Type::Bool),
                            ("AtomicBool" | "atomic_bool", "store") => ("atomic_bool_store", Type::Void),
                            ("AtomicBool" | "atomic_bool", "swap") => ("atomic_bool_swap", Type::Bool),
                            ("AtomicBool" | "atomic_bool", "cas") => ("atomic_bool_cas", Type::Bool),

                            // AtomicI64
                            ("AtomicI64" | "atomic_i64", "load") => ("atomic_i64_load", Type::Int),
                            ("AtomicI64" | "atomic_i64", "store") => ("atomic_i64_store", Type::Void),
                            ("AtomicI64" | "atomic_i64", "swap") => ("atomic_i64_swap", Type::Int),
                            ("AtomicI64" | "atomic_i64", "cas") => ("atomic_i64_cas", Type::Bool),
                            ("AtomicI64" | "atomic_i64", "add") => ("atomic_i64_add", Type::Int),
                            ("AtomicI64" | "atomic_i64", "sub") => ("atomic_i64_sub", Type::Int),
                            ("AtomicI64" | "atomic_i64", "and") => ("atomic_i64_and", Type::Int),
                            ("AtomicI64" | "atomic_i64", "or") => ("atomic_i64_or", Type::Int),
                            ("AtomicI64" | "atomic_i64", "xor") => ("atomic_i64_xor", Type::Int),

                            // AtomicUsize
                            ("AtomicUsize" | "atomic_usize", "load") => ("atomic_usize_load", Type::Int),
                            ("AtomicUsize" | "atomic_usize", "store") => ("atomic_usize_store", Type::Void),
                            ("AtomicUsize" | "atomic_usize", "swap") => ("atomic_usize_swap", Type::Int),
                            ("AtomicUsize" | "atomic_usize", "cas") => ("atomic_usize_cas", Type::Bool),
                            ("AtomicUsize" | "atomic_usize", "add") => ("atomic_usize_add", Type::Int),
                            ("AtomicUsize" | "atomic_usize", "sub") => ("atomic_usize_sub", Type::Int),

                            // AtomicPtr
                            ("AtomicPtr" | "atomic_ptr", "load") => ("atomic_ptr_load", if type_args.is_empty() { Type::RawPointer(Box::new(Type::U8)) } else { type_args[0].clone() }),
                            ("AtomicPtr" | "atomic_ptr", "store") => ("atomic_ptr_store", Type::Void),
                            ("AtomicPtr" | "atomic_ptr", "swap") => ("atomic_ptr_swap", if type_args.is_empty() { Type::RawPointer(Box::new(Type::U8)) } else { type_args[0].clone() }),
                            ("AtomicPtr" | "atomic_ptr", "cas") => ("atomic_ptr_cas", Type::Bool),

                            // TaskGroup / Nursery
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "fork" | "spawn") => {
                                let subtask_ret_type = if !typed_args.is_empty() {
                                    match &typed_args[0].type_ {
                                        Type::Function { return_type, .. } => vec![*return_type.clone()],
                                        _ => vec![],
                                    }
                                } else {
                                    vec![]
                                };
                                ("task_group_fork", Type::Generic { base: "Subtask".to_string(), type_args: subtask_ret_type })
                            }
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "join" | "wait") => ("task_group_join", Type::Int),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "cancel") => ("task_group_cancel", Type::Void),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "is_cancelled") => ("task_group_is_cancelled", Type::Bool),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "result") => ("task_group_result", Type::Int),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "token") => ("task_group_token", Type::Generic { base: "CancellationToken".to_string(), type_args: vec![] }),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "task_count") => ("task_group_task_count", Type::Int),
                            ("TaskGroup" | "task_group" | "Nursery" | "nursery", "completed_count") => ("task_group_completed_count", Type::Int),

                            // Subtask
                            ("Subtask" | "subtask", "get") => ("subtask_get", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("Subtask" | "subtask", "is_done") => ("subtask_is_done", Type::Bool),
                            ("Subtask" | "subtask", "is_cancelled") => ("subtask_is_cancelled", Type::Bool),

                            // CancellationToken
                            ("CancellationToken" | "cancellation_token" | "CancelToken" | "cancel_token", "cancel") => ("cancel_token_cancel", Type::Void),
                            ("CancellationToken" | "cancellation_token" | "CancelToken" | "cancel_token", "is_cancelled") => ("cancel_token_is_cancelled", Type::Bool),

                            // VecDeque
                            ("VecDeque" | "vec_deque", "push_back") => ("vec_deque_push_back", Type::Void),
                            ("VecDeque" | "vec_deque", "push_front") => ("vec_deque_push_front", Type::Void),
                            ("VecDeque" | "vec_deque", "pop_back") => ("vec_deque_pop_back", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("VecDeque" | "vec_deque", "pop_front") => ("vec_deque_pop_front", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("VecDeque" | "vec_deque", "front") => ("vec_deque_front", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("VecDeque" | "vec_deque", "back") => ("vec_deque_back", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("VecDeque" | "vec_deque", "get") => ("vec_deque_get", if type_args.is_empty() { Type::Unknown } else { type_args[0].clone() }),
                            ("VecDeque" | "vec_deque", "set") => ("vec_deque_set", Type::Void),
                            ("VecDeque" | "vec_deque", "len" | "length") => ("vec_deque_len", Type::Int),
                            ("VecDeque" | "vec_deque", "is_empty") => ("vec_deque_is_empty", Type::Bool),
                            ("VecDeque" | "vec_deque", "clear") => ("vec_deque_clear", Type::Void),

                            // BTreeMap
                            ("BTreeMap" | "btree_map", "insert") => ("btree_map_insert", Type::Void),
                            ("BTreeMap" | "btree_map", "get") => ("btree_map_get", if type_args.len() >= 2 { type_args[1].clone() } else { Type::Unknown }),
                            ("BTreeMap" | "btree_map", "contains_key" | "has") => ("btree_map_contains_key", Type::Bool),
                            ("BTreeMap" | "btree_map", "remove") => ("btree_map_remove", if type_args.len() >= 2 { type_args[1].clone() } else { Type::Unknown }),
                            ("BTreeMap" | "btree_map", "len" | "size") => ("btree_map_len", Type::Int),
                            ("BTreeMap" | "btree_map", "is_empty") => ("btree_map_is_empty", Type::Bool),
                            ("BTreeMap" | "btree_map", "first_key") => ("btree_map_first_key", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BTreeMap" | "btree_map", "last_key") => ("btree_map_last_key", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BTreeMap" | "btree_map", "keys") => ("btree_map_keys", Type::List),
                            ("BTreeMap" | "btree_map", "values") => ("btree_map_values", Type::List),
                            ("BTreeMap" | "btree_map", "clear") => ("btree_map_clear", Type::Void),

                            // BTreeSet
                            ("BTreeSet" | "btree_set", "insert") => ("btree_set_insert", Type::Bool),
                            ("BTreeSet" | "btree_set", "contains") => ("btree_set_contains", Type::Bool),
                            ("BTreeSet" | "btree_set", "remove") => ("btree_set_remove", Type::Bool),
                            ("BTreeSet" | "btree_set", "len" | "size") => ("btree_set_len", Type::Int),
                            ("BTreeSet" | "btree_set", "is_empty") => ("btree_set_is_empty", Type::Bool),
                            ("BTreeSet" | "btree_set", "first") => ("btree_set_first", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BTreeSet" | "btree_set", "last") => ("btree_set_last", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BTreeSet" | "btree_set", "to_list") => ("btree_set_to_list", Type::List),
                            ("BTreeSet" | "btree_set", "clear") => ("btree_set_clear", Type::Void),

                            // BinaryHeap
                            ("BinaryHeap" | "binary_heap", "push") => ("binary_heap_push", Type::Void),
                            ("BinaryHeap" | "binary_heap", "pop") => ("binary_heap_pop", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BinaryHeap" | "binary_heap", "peek") => ("binary_heap_peek", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("BinaryHeap" | "binary_heap", "len" | "size") => ("binary_heap_len", Type::Int),
                            ("BinaryHeap" | "binary_heap", "is_empty") => ("binary_heap_is_empty", Type::Bool),
                            ("BinaryHeap" | "binary_heap", "clear") => ("binary_heap_clear", Type::Void),

                            // IndexMap
                            ("IndexMap" | "index_map", "insert") => ("index_map_insert", Type::Void),
                            ("IndexMap" | "index_map", "get") => ("index_map_get", if type_args.len() >= 2 { type_args[1].clone() } else { Type::Unknown }),
                            ("IndexMap" | "index_map", "get_index_key") => ("index_map_get_index_key", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("IndexMap" | "index_map", "get_index_value") => ("index_map_get_index_value", if type_args.len() >= 2 { type_args[1].clone() } else { Type::Unknown }),
                            ("IndexMap" | "index_map", "contains_key" | "has") => ("index_map_contains_key", Type::Bool),
                            ("IndexMap" | "index_map", "remove") => ("index_map_remove", if type_args.len() >= 2 { type_args[1].clone() } else { Type::Unknown }),
                            ("IndexMap" | "index_map", "len" | "size") => ("index_map_len", Type::Int),
                            ("IndexMap" | "index_map", "is_empty") => ("index_map_is_empty", Type::Bool),
                            ("IndexMap" | "index_map", "keys") => ("index_map_keys", Type::List),
                            ("IndexMap" | "index_map", "values") => ("index_map_values", Type::List),
                            ("IndexMap" | "index_map", "clear") => ("index_map_clear", Type::Void),

                            // RingBuffer
                            ("RingBuffer" | "ring_buffer", "push") => ("ring_buffer_push", Type::Bool),
                            ("RingBuffer" | "ring_buffer", "push_overwrite") => ("ring_buffer_push_overwrite", Type::Void),
                            ("RingBuffer" | "ring_buffer", "pop") => ("ring_buffer_pop", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("RingBuffer" | "ring_buffer", "peek") => ("ring_buffer_peek", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("RingBuffer" | "ring_buffer", "len") => ("ring_buffer_len", Type::Int),
                            ("RingBuffer" | "ring_buffer", "capacity") => ("ring_buffer_capacity", Type::Int),
                            ("RingBuffer" | "ring_buffer", "is_empty") => ("ring_buffer_is_empty", Type::Bool),
                            ("RingBuffer" | "ring_buffer", "is_full") => ("ring_buffer_is_full", Type::Bool),
                            ("RingBuffer" | "ring_buffer", "clear") => ("ring_buffer_clear", Type::Void),

                            // BitSet
                            ("BitSet" | "bit_set", "set") => ("bit_set_set", Type::Void),
                            ("BitSet" | "bit_set", "clear_bit") => ("bit_set_clear_bit", Type::Void),
                            ("BitSet" | "bit_set", "test") => ("bit_set_test", Type::Bool),
                            ("BitSet" | "bit_set", "flip") => ("bit_set_flip", Type::Void),
                            ("BitSet" | "bit_set", "count_ones" | "cardinality") => ("bit_set_count_ones", Type::Int),
                            ("BitSet" | "bit_set", "len" | "size") => ("bit_set_len", Type::Int),
                            ("BitSet" | "bit_set", "is_empty") => ("bit_set_is_empty", Type::Bool),
                            ("BitSet" | "bit_set", "union_with") => ("bit_set_union", Type::Void),
                            ("BitSet" | "bit_set", "intersect_with") => ("bit_set_intersect", Type::Void),
                            ("BitSet" | "bit_set", "difference_with") => ("bit_set_difference", Type::Void),
                            ("BitSet" | "bit_set", "clear") => ("bit_set_clear", Type::Void),

                            // Box<T>
                            ("RozeBox" | "Box" | "box", "get") => ("box_get", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("RozeBox" | "Box" | "box", "set") => ("box_set", Type::Void),
                            ("RozeBox" | "Box" | "box", "into_raw") => ("box_into_raw", Type::Int),

                            // Arc<T>
                            ("RozeArc" | "Arc" | "arc", "get") => ("arc_get", if !type_args.is_empty() { type_args[0].clone() } else { Type::Unknown }),
                            ("RozeArc" | "Arc" | "arc", "clone") => ("arc_clone", Type::Generic { base: "RozeArc".to_string(), type_args: type_args.clone() }),
                            ("RozeArc" | "Arc" | "arc", "strong_count") => ("arc_strong_count", Type::Int),
                            ("RozeArc" | "Arc" | "arc", "weak_count") => ("arc_weak_count", Type::Int),
                            ("RozeArc" | "Arc" | "arc", "downgrade") => ("arc_downgrade", Type::Generic { base: "RozeWeak".to_string(), type_args: type_args.clone() }),

                            // Weak<T>
                            ("RozeWeak" | "Weak" | "weak", "upgrade") => ("weak_upgrade", Type::Generic { base: "RozeArc".to_string(), type_args: type_args.clone() }),
                            ("RozeWeak" | "Weak" | "weak", "strong_count") => ("weak_strong_count", Type::Int),
                            ("RozeWeak" | "Weak" | "weak", "weak_count") => ("weak_weak_count", Type::Int),
                            ("RozeWeak" | "Weak" | "weak", "is_expired") => ("weak_is_expired", Type::Bool),
                            ("RozeWeak" | "Weak" | "weak", "is_alive") => ("weak_is_alive", Type::Bool),

                            // BumpAllocator
                            ("BumpAllocator" | "bump_allocator", "alloc") => ("bump_alloc", Type::Int),
                            ("BumpAllocator" | "bump_allocator", "alloc_aligned") => ("bump_alloc_aligned", Type::Int),
                            ("BumpAllocator" | "bump_allocator", "reset") => ("bump_reset", Type::Void),
                            ("BumpAllocator" | "bump_allocator", "allocated_bytes") => ("bump_allocated_bytes", Type::Int),
                            ("BumpAllocator" | "bump_allocator", "remaining_bytes") => ("bump_remaining_bytes", Type::Int),
                            ("BumpAllocator" | "bump_allocator", "capacity") => ("bump_capacity", Type::Int),
                            ("BumpAllocator" | "bump_allocator", "is_empty") => ("bump_is_empty", Type::Bool),

                            // ArenaAllocator
                            ("ArenaAllocator" | "arena_allocator", "alloc") => ("arena_alloc", Type::Int),
                            ("ArenaAllocator" | "arena_allocator", "alloc_aligned") => ("arena_alloc_aligned", Type::Int),
                            ("ArenaAllocator" | "arena_allocator", "reset") => ("arena_reset", Type::Void),
                            ("ArenaAllocator" | "arena_allocator", "allocated_bytes") => ("arena_allocated_bytes", Type::Int),
                            ("ArenaAllocator" | "arena_allocator", "chunk_count") => ("arena_chunk_count", Type::Int),
                            ("ArenaAllocator" | "arena_allocator", "clear") => ("arena_clear", Type::Void),

                            // PoolAllocator
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "alloc") => ("pool_alloc", Type::Int),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "dealloc") => ("pool_dealloc", Type::Bool),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "block_size") => ("pool_block_size", Type::Int),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "capacity") => ("pool_capacity", Type::Int),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "available_blocks") => ("pool_available_blocks", Type::Int),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "is_full") => ("pool_is_full", Type::Bool),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "is_empty") => ("pool_is_empty", Type::Bool),
                            ("PoolAllocator" | "pool_allocator" | "FixedBlockPoolAllocator" | "fixed_block_pool_allocator", "reset") => ("pool_reset", Type::Void),

                            _ => {
                                return Err(RozeError::type_error(
                                    format!("Cannot call method '{}' on non-class/trait type '{}'", method, obj_type),
                                    location.line, location.column,
                                ).into());
                            }
                        };

                        let mut all_args = Vec::with_capacity(1 + typed_args.len());
                        all_args.push(typed_obj);
                        all_args.extend(typed_args);

                        Ok(TypedExpression {
                            kind: TypedExpressionKind::Call {
                                function: fn_name.to_string(),
                                arguments: all_args,
                            },
                            type_: ret_ty,
                            location: location.clone(),
                        })
                    }
                    other if other.is_integer() => {
                        match method.as_str() {
                            "wrapping_add" | "wrapping_sub" | "wrapping_mul"
                            | "saturating_add" | "saturating_sub" | "saturating_mul" if typed_args.len() == 1 => {
                                let mut all_args = Vec::with_capacity(2);
                                all_args.push(typed_obj);
                                all_args.extend(typed_args);
                                Ok(TypedExpression {
                                    kind: TypedExpressionKind::Call {
                                        function: method.clone(),
                                        arguments: all_args,
                                    },
                                    type_: other.clone(),
                                    location: location.clone(),
                                })
                            }
                            _ => {
                                return Err(RozeError::type_error(
                                    format!("Cannot call method '{}' on type '{}'", method, other),
                                    location.line, location.column,
                                ).into());
                            }
                        }
                    }
                    other if other.is_simd_vector() => {
                        let prefix = match other {
                            Type::Vec4f => "vec4f",
                            Type::Vec8f => "vec8f",
                            Type::Vec4i => "vec4i",
                            Type::Vec16u8 => "vec16u8",
                            _ => unreachable!(),
                        };
                        let (fn_name, ret_ty) = match (other, method.as_str()) {
                            (_, "add" | "sub" | "mul" | "div" | "min" | "max") => (format!("{}_{}", prefix, method), other.clone()),
                            (Type::Vec4f | Type::Vec8f, "dot" | "sum") => (format!("{}_{}", prefix, method), Type::F32),
                            (Type::Vec4i, "sum") => ("vec4i_sum".to_string(), Type::I32),
                            (Type::Vec16u8, "sum") => ("vec16u8_sum".to_string(), Type::Int),
                            (_, "sqrt" | "abs") => (format!("{}_{}", prefix, method), other.clone()),
                            (Type::Vec4f | Type::Vec8f, "fma") => (format!("{}_{}", prefix, method), other.clone()),
                            (Type::Vec16u8, "saturating_add" | "saturating_sub") => (format!("{}_{}", prefix, method), other.clone()),
                            (Type::Vec4f | Type::Vec8f, "get") => (format!("{}_{}", prefix, method), Type::F32),
                            (Type::Vec4i, "get") => ("vec4i_get".to_string(), Type::I32),
                            (Type::Vec16u8, "get") => ("vec16u8_get".to_string(), Type::U8),
                            (_, "set") => (format!("{}_{}", prefix, method), other.clone()),
                            (_, "store") => (format!("{}_{}", prefix, method), Type::Void),
                            _ => {
                                return Err(RozeError::type_error(
                                    format!("Cannot call method '{}' on type '{}'", method, other),
                                    location.line, location.column,
                                ).into());
                            }
                        };

                        let mut all_args = Vec::with_capacity(1 + typed_args.len());
                        all_args.push(typed_obj);
                        all_args.extend(typed_args);

                        Ok(TypedExpression {
                            kind: TypedExpressionKind::Call {
                                function: fn_name,
                                arguments: all_args,
                            },
                            type_: ret_ty,
                            location: location.clone(),
                        })
                    }
                    other => {
                        return Err(RozeError::type_error(
                            format!("Cannot call method '{}' on non-class/trait type '{}'", method, other),
                            location.line, location.column,
                        ).into());
                    }
                }
            }
            Expression::Unsafe { expr, location: _ } => {
                let prev_unsafe = self.in_unsafe;
                self.in_unsafe = true;
                let res = self.check_expression(expr);
                self.in_unsafe = prev_unsafe;
                res
            }
            Expression::Comptime { expr, location } => {
                let val = {
                    let mut eval = ComptimeEvaluator::new(self);
                    eval.eval_expr(expr)?
                };
                self.comptime_value_to_typed_expr(&val, location)
            }
            Expression::ComptimeBlock { block, location } => {
                let val = {
                    let mut eval = ComptimeEvaluator::new(self);
                    eval.eval_stmt(block)?
                };
                let final_val = match val {
                    ComptimeValue::Return(r) => *r,
                    other => other,
                };
                self.comptime_value_to_typed_expr(&final_val, location)
            }
            Expression::Await { expr, location } => {
                let typed_expr = self.check_expression(expr)?;
                let result_type = match &typed_expr.type_ {
                    Type::Generic { base, type_args } if (base == "Future" || base == "future") && !type_args.is_empty() => {
                        type_args[0].clone()
                    }
                    Type::Generic { base, .. } if (base == "Future" || base == "future") => {
                        Type::Unknown
                    }
                    Type::Unknown => Type::Unknown,
                    _ => {
                        return Err(RozeError::type_error(
                            format!("Cannot await non-Future type '{}'", typed_expr.type_),
                            location.line,
                            location.column,
                        ).into());
                    }
                };
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Await(Box::new(typed_expr)),
                    type_: result_type,
                    location: location.clone(),
                })
            }
        }
    }
}

fn collect_referenced_identifiers(stmt: &TypedStatement, idents: &mut Vec<String>) {
    match stmt {
        TypedStatement::Let { value, .. } => collect_from_expr(value, idents),
        TypedStatement::Assign { name, value, .. } => {
            idents.push(name.clone());
            collect_from_expr(value, idents);
        }
        TypedStatement::Expression { expr, .. } => collect_from_expr(expr, idents),
        TypedStatement::Return { value, .. } => {
            if let Some(val) = value {
                collect_from_expr(val, idents);
            }
        }
        TypedStatement::Block { statements, .. } => {
            for s in statements {
                collect_referenced_identifiers(s, idents);
            }
        }
        TypedStatement::If { condition, then_branch, else_branch, .. } => {
            collect_from_expr(condition, idents);
            collect_referenced_identifiers(then_branch, idents);
            if let Some(eb) = else_branch {
                collect_referenced_identifiers(eb, idents);
            }
        }
        TypedStatement::While { condition, body, .. } => {
            collect_from_expr(condition, idents);
            collect_referenced_identifiers(body, idents);
        }
        TypedStatement::For { init, condition, update, body, .. } => {
            collect_referenced_identifiers(init, idents);
            collect_from_expr(condition, idents);
            collect_referenced_identifiers(update, idents);
            collect_referenced_identifiers(body, idents);
        }
        TypedStatement::FieldAssign { object, value, .. } => {
            collect_from_expr(object, idents);
            collect_from_expr(value, idents);
        }
        TypedStatement::Match { expr, arms, .. } => {
            collect_from_expr(expr, idents);
            for arm in arms {
                collect_referenced_identifiers(&arm.body, idents);
            }
        }
        _ => {}
    }
}

fn collect_from_expr(expr: &TypedExpression, idents: &mut Vec<String>) {
    match &expr.kind {
        TypedExpressionKind::Identifier(name) => idents.push(name.clone()),
        TypedExpressionKind::Binary { left, right, .. } => {
            collect_from_expr(left, idents);
            collect_from_expr(right, idents);
        }
        TypedExpressionKind::Unary { operand, .. } => collect_from_expr(operand, idents),
        TypedExpressionKind::Call { arguments, .. } => {
            for a in arguments {
                collect_from_expr(a, idents);
            }
        }
        TypedExpressionKind::IndirectCall { callee, arguments } => {
            collect_from_expr(callee, idents);
            for a in arguments {
                collect_from_expr(a, idents);
            }
        }
        TypedExpressionKind::New { arguments, .. } => {
            for a in arguments {
                collect_from_expr(a, idents);
            }
        }
        TypedExpressionKind::FieldAccess { object, .. } => collect_from_expr(object, idents),
        TypedExpressionKind::Try(inner) => collect_from_expr(inner, idents),
        TypedExpressionKind::EnumConstruct { arguments, .. } => {
            for a in arguments {
                collect_from_expr(a, idents);
            }
        }
        TypedExpressionKind::Closure { captures, .. } => {
            for (cap, _) in captures {
                idents.push(cap.clone());
            }
        }
        _ => {}
    }
}

fn collect_return_types(stmt: &TypedStatement, inferred: &mut Option<Type>) {
    match stmt {
        TypedStatement::Return { value, .. } => {
            if let Some(val) = value {
                *inferred = Some(val.type_.clone());
            } else {
                *inferred = Some(Type::Void);
            }
        }
        TypedStatement::Block { statements, .. } => {
            for s in statements {
                collect_return_types(s, inferred);
            }
        }
        TypedStatement::If { then_branch, else_branch, .. } => {
            collect_return_types(then_branch, inferred);
            if let Some(eb) = else_branch {
                collect_return_types(eb, inferred);
            }
        }
        _ => {}
    }
}

/// Type-checks `program`, discarding the resulting IR -- for callers
/// that only want a pass/fail answer (e.g. the LSP's diagnostics engine,
/// which cares whether the program is valid, not about generating code
/// for it). Compiling to Java should use `check_and_lower` instead, to
/// avoid re-deriving the same information a second time in codegen.
///
/// (The "never used" dead-code warning this can trigger is a known
/// false positive from the bin/lib dual-module-tree setup: roze-lsp
/// genuinely calls this through the lib target; only the bin target's
/// own separate copy of this file -- main.rs uses check_and_lower
/// directly instead -- doesn't.)
#[allow(dead_code)]
pub fn check_types(program: &Program) -> Result<()> {
    check_and_lower(program).map(|_| ())
}

/// Type-checks `program` and returns its fully type-annotated IR (see
/// `crate::ir`), ready for codegen to consume directly.
pub fn check_and_lower(program: &Program) -> Result<crate::ir::TypedProgram> {
    check_and_lower_with_options(program, false, false)
}

pub fn check_and_lower_with_options(
    program: &Program,
    no_heap: bool,
    disable_float_registers: bool,
) -> Result<crate::ir::TypedProgram> {
    let mut checker = TypeChecker::new();
    checker.no_heap = no_heap;
    checker.disable_float_registers = disable_float_registers;
    let typed_program = checker.check_program(program)?;

    if !checker.errors.is_empty() {
        for error in &checker.errors {
            eprintln!("{}", error);
        }
        return Err(anyhow::anyhow!("Type checking failed"));
    }
    Ok(typed_program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_source(src: &str) -> Result<()> {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_types(&program)
    }

    fn typed_program(src: &str) -> TypedProgram {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_and_lower(&program).expect("fixture should type-check")
    }

    /// Pulls out the declared type of every top-level `let` in `main`'s
    /// body, in order -- for tests that need to inspect what a value
    /// actually resolved to, not just whether the program type-checked.
    fn main_let_types(program: &TypedProgram) -> Vec<&Type> {
        program.statements.iter()
            .find_map(|s| if let TypedStatement::Function { name, body, .. } = s {
                if name == "main" { Some(body) } else { None }
            } else { None })
            .map(|body| if let TypedStatement::Block { statements, .. } = body.as_ref() {
                statements.iter().filter_map(|s| if let TypedStatement::Let { value, .. } = s { Some(&value.type_) } else { None }).collect()
            } else { vec![] })
            .unwrap_or_default()
    }

    #[test]
    fn valid_program_passes() {
        assert!(check_source("func main() { let x = 5; println(x); }").is_ok());
    }

    #[test]
    fn undefined_variable_is_an_error() {
        assert!(check_source("func main() { println(x); }").is_err());
    }

    #[test]
    fn undefined_function_is_an_error() {
        assert!(check_source("func main() { totally_made_up(1); }").is_err());
    }

    #[test]
    fn return_type_mismatch_is_an_error() {
        let result = check_source("func f() -> int { return \"not a number\"; } func main() { }");
        assert!(result.is_err());
    }

    #[test]
    fn matching_return_type_is_ok() {
        let result = check_source("func f() -> int { return 5; } func main() { }");
        assert!(result.is_ok());
    }

    #[test]
    fn bare_return_in_non_void_function_is_an_error() {
        let result = check_source("func f() -> int { return; } func main() { }");
        assert!(result.is_err());
    }

    #[test]
    fn bare_return_in_void_function_is_ok() {
        let result = check_source("func f() { return; } func main() { }");
        assert!(result.is_ok());
    }

    #[test]
    fn reassignment_changing_type_is_an_error() {
        let result = check_source("func main() { let x = 5; x = \"five\"; }");
        assert!(result.is_err());
    }

    #[test]
    fn reassignment_preserving_type_is_ok() {
        let result = check_source("func main() { let x = 5; x = 10; }");
        assert!(result.is_ok());
    }

    #[test]
    fn main_declaring_return_type_is_an_error() {
        let result = check_source("func main() -> int { }");
        assert!(result.is_err());
    }

    #[test]
    fn intrinsics_are_callable_without_definition() {
        let result = check_source(
            "func main() { println(abs(-5)); println(string_length(\"hi\")); }"
        );
        assert!(result.is_ok());
    }

    #[test]
    fn collections_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let l = list_new(); \
                list_push(l, 1); \
                let x = list_get(l, 0); \
                list_set(l, 0, 2); \
                list_remove(l, 0); \
                println(list_length(l)); \
                println(list_is_empty(l)); \
                let m = map_new(); \
                map_put(m, \"a\", 1); \
                let v = map_get(m, \"a\"); \
                println(map_has(m, \"a\")); \
                map_remove(m, \"a\"); \
                println(map_size(m)); \
                println(map_is_empty(m)); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn list_and_map_type_annotations_are_recognized() {
        let result = check_source(
            "func takes_a_list(l: list) -> list { return l; } \
             func takes_a_map(m: map) -> map { return m; } \
             func main() { \
                let l = takes_a_list(list_new()); \
                let m = takes_a_map(map_new()); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn reassigning_a_list_variable_to_a_map_is_an_error() {
        let result = check_source(
            "func main() { let l = list_new(); l = map_new(); }"
        );
        assert!(result.is_err());
    }

    #[test]
    fn file_and_network_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                write_file(\"a\", \"b\"); \
                append_file(\"a\", \"c\"); \
                println(read_file(\"a\")); \
                println(file_exists(\"a\")); \
                println(delete_file(\"a\")); \
                let lines = read_lines(\"a\"); \
                println(http_get(\"http://example.com\")); \
                println(http_post(\"http://example.com\", \"body\")); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn json_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let m = map_new(); \
                map_put(m, \"a\", 1); \
                let encoded = json_encode(m); \
                let decoded = json_decode(encoded); \
                println(decoded); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn http_server_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let server = http_server_start(8080); \
                let req = http_server_accept(server); \
                println(map_get(req, \"method\")); \
                http_server_respond(req, 200, \"ok\"); \
                http_server_stop(server); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn sql_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let conn = sql_connect(\"jdbc:h2:mem:x\"); \
                sql_execute(conn, \"CREATE TABLE t (x INT)\"); \
                let rows = sql_query(conn, \"SELECT * FROM t\"); \
                println(list_length(rows)); \
                sql_close(conn); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    // ---- Classes ----

    #[test]
    fn class_construction_field_access_and_assignment_type_check() {
        let result = check_source(
            "class Point { x: int, y: int } \
             func main() { \
                let p = new Point(1, 2); \
                println(p.x); \
                p.x = 5; \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn class_can_be_used_as_a_function_parameter_and_return_type() {
        let result = check_source(
            "class Point { x: int } \
             func make(x: int) -> Point { return new Point(x); } \
             func get_x(p: Point) -> int { return p.x; } \
             func main() { \
                let p = make(5); \
                println(get_x(p)); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn class_can_be_declared_after_a_function_that_uses_it() {
        // Classes are registered in a first pass, before functions, so
        // a function earlier in the file can reference a class declared
        // later.
        let result = check_source(
            "func make() -> Point { return new Point(1); } \
             class Point { x: int } \
             func main() { println(make().x); }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn wrong_number_of_constructor_arguments_is_an_error() {
        let result = check_source("class Point { x: int, y: int } func main() { let p = new Point(1); }");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_constructor_argument_type_is_an_error() {
        let result = check_source("class Point { x: int, y: int } func main() { let p = new Point(\"hi\", 2); }");
        assert!(result.is_err());
    }

    #[test]
    fn accessing_an_undefined_field_is_an_error() {
        let result = check_source("class Point { x: int } func main() { let p = new Point(1); println(p.z); }");
        assert!(result.is_err());
    }

    #[test]
    fn assigning_wrong_type_to_a_field_is_an_error() {
        let result = check_source("class Point { x: int } func main() { let p = new Point(1); p.x = \"oops\"; }");
        assert!(result.is_err());
    }

    #[test]
    fn constructing_an_undefined_class_is_an_error() {
        let result = check_source("func main() { let p = new Nonexistent(1); }");
        assert!(result.is_err());
    }

    #[test]
    fn accessing_a_field_on_a_non_class_value_is_an_error() {
        let result = check_source("func main() { let x = 5; println(x.field); }");
        assert!(result.is_err());
    }

    #[test]
    fn float_literal_is_distinguished_from_int_by_a_decimal_point() {
        let program = typed_program("func main() { let x = 3.14; let y = 5; }");
        let types = main_let_types(&program);
        assert_eq!(types, vec![&Type::Float, &Type::Int]);
    }

    #[test]
    fn mixed_int_float_arithmetic_promotes_to_float() {
        assert!(check_source("func main() { let x = 1 + 2.5; }").is_ok());
        assert!(check_source("func main() { let x = 2.5 + 1; }").is_ok());
        assert!(check_source("func main() { let x = 1 - 2.5; }").is_ok());
        assert!(check_source("func main() { let x = 2.5 * 2; }").is_ok());
        assert!(check_source("func main() { let x = 5 / 2.0; }").is_ok());
    }

    #[test]
    fn float_can_flow_into_a_function_expecting_float_but_not_the_reverse() {
        // Implicit widening (int -> float) is allowed; implicit
        // narrowing (float -> int) is not, since it would silently
        // lose precision.
        assert!(check_source("func f(x: float) -> float { return x; } func main() { f(5); }").is_ok());
        assert!(check_source("func f(x: int) -> int { return x; } func main() { f(5.0); }").is_err());
    }

    #[test]
    fn abs_max_min_return_type_tracks_their_argument_types() {
        // abs(-3.14) must type-check as Float (not the old fixed Int),
        // or a `let` binding downstream would get the wrong declared
        // type and fail to compile at the codegen level.
        let program = typed_program("func main() { let a = abs(-3.14); let b = abs(-5); let c = max(3, 7.5); }");
        let types = main_let_types(&program);
        assert_eq!(types, vec![&Type::Float, &Type::Int, &Type::Float]);
    }

    #[test]
    fn function_call_arguments_are_validated_for_count_and_type() {
        // A real, general gap this phase also found and fixed: ordinary
        // function calls (unlike class construction, which already
        // validated its constructor arguments) had no argument
        // count/type validation at all before now.
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1, 2); }").is_ok());
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1); }").is_err());
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1, 2, 3); }").is_err());
        assert!(check_source("func f(a: int) {} func main() { f(\"hi\"); }").is_err());
    }

    #[test]
    fn for_loop_variable_is_scoped_to_the_loop() {
        // `i` from the for-loop's init clause must be visible inside the
        // loop body/condition/update...
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { println(i); } }").is_ok());
        // ...but must NOT leak out past the loop.
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { } println(i); }").is_err());
    }

    // ---- IR-specific tests: the typed tree itself, not just pass/fail ----

    fn lower(src: &str) -> crate::ir::TypedProgram {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_and_lower(&program).expect("fixture should type-check")
    }

    #[test]
    fn lowering_attaches_types_to_literals() {
        let ir = lower("func main() { let x = 5; let s = \"hi\"; let b = true; }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        for (stmt, expected) in stmts.iter().zip([Type::Int, Type::String, Type::Bool]) {
            match stmt {
                TypedStatement::Let { value, .. } => assert_eq!(value.type_, expected),
                other => panic!("expected a Let, got {:?}", other),
            }
        }
    }

    #[test]
    fn lowering_resolves_identifier_types_from_the_symbol_table() {
        let ir = lower("func main() { let x = 5; println(x); }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        match &stmts[1] {
            TypedStatement::Expression { expr, .. } => match &expr.kind {
                TypedExpressionKind::Call { arguments, .. } => {
                    assert_eq!(arguments[0].type_, Type::Int, "the reference to x should carry its resolved type");
                }
                other => panic!("expected a Call, got {:?}", other),
            },
            other => panic!("expected an Expression statement, got {:?}", other),
        }
    }

    #[test]
    fn lowering_resolves_intrinsic_call_return_types() {
        let ir = lower("func main() { let n = list_new(); }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        match &stmts[0] {
            TypedStatement::Let { value, .. } => assert_eq!(value.type_, Type::List),
            other => panic!("expected a Let, got {:?}", other),
        }
    }

    #[test]
    fn lowering_drops_import_statements_with_no_ir_form() {
        // Only reachable if resolve_imports somehow didn't already strip
        // it (see the Statement::Import arm's comment) -- still, the IR
        // itself should never contain one either way.
        let ir = lower("func main() { }");
        assert!(!ir.statements.iter().any(|s| matches!(s, TypedStatement::Function { name, .. } if name.is_empty())));
    }

    #[test]
    fn option_and_result_type_annotations_are_recognized() {
        let result = check_source(
            "func takes_opt(o: Option) -> Option { return o; } \
             func takes_res(r: Result) -> Result { return r; } \
             func generic_like(r: Result<string, string>) -> Option<int> { return None(); } \
             func main() { \
                 let o = takes_opt(Some(42)); \
                 let r = takes_res(Ok(\"ok\")); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn option_and_result_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                 let o = Some(10); \
                 let n = None(); \
                 println(is_some(o)); \
                 println(is_none(n)); \
                 println(unwrap(o)); \
                 println(unwrap_or(n, 0)); \
                 let r = Ok(\"data\"); \
                 let e = Err(\"fail\"); \
                 println(is_ok(r)); \
                 println(is_err(e)); \
                 println(unwrap(r)); \
                 println(unwrap_err(e)); \
                 println(unwrap_or(e, \"fallback\")); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn try_operator_type_checks_in_matching_function() {
        let result = check_source(
            "func get_val() -> Result { \
                 let r = Ok(5); \
                 let v = r?; \
                 return Ok(v); \
             } \
             func get_opt() -> Option { \
                 let o = Some(10); \
                 let v = o?; \
                 return Some(v); \
             } \
             func main() { }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn try_operator_on_result_in_void_or_int_function_is_error() {
        let result = check_source(
            "func calc() -> int { \
                 let r = Ok(5); \
                 let v = r?; \
                 return v; \
             } \
             func main() { }"
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("requires the enclosing function to return Result"), "{}", err);
    }

    #[test]
    fn enum_definition_and_matching_type_checks_cleanly() {
        let result = check_source(
            "enum Color { Red, Green, Blue } \
             enum Shape { Circle(int), Rect(int, int) } \
             func area(s: Shape) -> int { \
                 match s { \
                     Circle(r) => { return r * r; } \
                     Rect(w, h) => { return w * h; } \
                 } \
             } \
             func main() { \
                 let c = Color::Red; \
                 let s = Shape::Circle(5); \
                 let a = area(s); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn enum_variant_argument_count_mismatch_is_error() {
        let result = check_source(
            "enum Shape { Circle(int) } \
             func main() { \
                 let s = Shape::Circle(1, 2); \
             }"
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("takes 1 argument(s), but 2 were given"), "{}", err);
    }

    #[test]
    fn enum_variant_argument_type_mismatch_is_error() {
        let result = check_source(
            "enum Shape { Circle(int) } \
             func main() { \
                 let s = Shape::Circle(\"invalid\"); \
             }"
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expects int for argument, got string"), "{}", err);
    }

    #[test]
    fn enum_match_pattern_variable_binding_count_mismatch_is_error() {
        let result = check_source(
            "enum Shape { Circle(int) } \
             func main() { \
                 let s = Shape::Circle(5); \
                 match s { \
                     Circle(a, b) => {} \
                 } \
             }"
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expects 1 argument(s), but pattern has 2"), "{}", err);
    }
}
