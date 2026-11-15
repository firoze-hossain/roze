# 🌹 The Roze Language Grammar Specification (v2.0.0 'Prometheus')

This document provides the authoritative, formal **Extended Backus-Naur Form (EBNF)** (ISO/IEC 14977) grammar specification for the **Roze Programming Language v2.0 ('Prometheus')**.

---

## 1. Notation and Conventions

| Notation | Meaning |
| :--- | :--- |
| `[ ... ]` | Optional element (0 or 1 time) |
| `{ ... }` | Repetition (0 or more times) |
| `( ... )` | Syntactic grouping |
| `\|` | Alternation (choice) |
| `" ... "` | Terminal literal token or character |
| `,` | Sequential concatenation |
| `-` | Set subtraction / exception |
| `;` | Rule termination |

---

## 2. Lexical Structure

### 2.1 Whitespace and Comments
```ebnf
Whitespace        = " " | "\t" | "\r" | "\n" ;
LineComment       = "//" , { AnyCharExceptNewline } , ( "\n" | EOF ) ;
DocLineComment    = "///" , { AnyCharExceptNewline } , ( "\n" | EOF ) ;
ModuleDocComment  = "//!" , { AnyCharExceptNewline } , ( "\n" | EOF ) ;
BlockComment      = "/*" , { AnyChar | BlockComment } , "*/" ;
Comment           = LineComment | DocLineComment | ModuleDocComment | BlockComment ;
```

### 2.2 Reserved Keywords
```ebnf
Keyword           = "func"     | "let"      | "var"      | "if"       | "else"
                  | "while"    | "for"      | "return"   | "class"    | "new"
                  | "extern"   | "import"   | "enum"     | "match"    | "trait"
                  | "impl"     | "unsafe"   | "null"     | "true"     | "false"
                  | "as"       | "in"       | "break"    | "continue" | "type"
                  | "where"    | "comptime" | "async"    | "await"    | "spawn"
                  | "channel"  | "send"     | "recv"     | "select"   | "asm"
                  | "yield"    | "defer"    | "nursery"  | "taskgroup" ;
```

### 2.3 Identifiers
```ebnf
Identifier        = ( Letter | "_" ) , { Letter | Digit | "_" } ;
Letter            = "a" ... "z" | "A" ... "Z" ;
Digit             = "0" ... "9" ;
HexDigit          = Digit | "a" ... "f" | "A" ... "F" ;
OctDigit          = "0" ... "7" ;
BinDigit          = "0" | "1" ;
```

### 2.4 Literals
```ebnf
Literal           = IntegerLiteral | FloatLiteral | StringLiteral | RawStringLiteral 
                  | CharLiteral | BooleanLiteral | NullLiteral ;

IntegerLiteral    = [ "-" ] , ( DecimalLiteral | HexLiteral | OctLiteral | BinLiteral ) , [ IntSuffix ] ;
DecimalLiteral    = Digit , { Digit | "_" } ;
HexLiteral        = "0" , ( "x" | "X" ) , HexDigit , { HexDigit | "_" } ;
OctLiteral        = "0" , ( "o" | "O" ) , OctDigit , { OctDigit | "_" } ;
BinLiteral        = "0" , ( "b" | "B" ) , BinDigit , { BinDigit | "_" } ;
IntSuffix         = "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" ;

FloatLiteral      = [ "-" ] , DecimalLiteral , "." , DecimalLiteral , [ Exponent ] , [ FloatSuffix ] ;
Exponent          = ( "e" | "E" ) , [ "+" | "-" ] , DecimalLiteral ;
FloatSuffix       = "f32" | "f64" ;

CharLiteral       = "'" , ( AnyCharExceptQuoteOrBackslash | EscapeSequence ) , "'" ;
StringLiteral     = "\"" , { StringChar | EscapeSequence } , "\"" ;
RawStringLiteral  = "r\"" , { AnyCharExceptQuote } , "\"" ;
StringChar        = AnyCharExceptQuoteOrBackslashOrNewline ;
EscapeSequence    = "\\" , ( "n" | "t" | "r" | "\\" | "\"" | "'" | "0" | "x" , HexDigit , HexDigit | "u{" , { HexDigit } , "}" ) ;

BooleanLiteral    = "true" | "false" ;
NullLiteral       = "null" ;
```

### 2.5 Operators and Punctuators
```ebnf
ArithmeticOp      = "+"  | "-"  | "*"  | "/"  | "%" ;
BitwiseOp         = "&"  | "|"  | "^"  | "~"  | "<<" | ">>" ;
ComparisonOp      = "==" | "!=" | "<"  | "<=" | ">"  | ">=" ;
LogicalOp         = "&&" | "||" | "!" ;
AssignmentOp      = "="  | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
ArrowOp           = "->" | "=>" ;
RangeOp           = ".." | "..=" ;
NullSafetyOp      = "?"  | "?." | "??" ;
Punctuator        = "("  | ")"  | "{"  | "}"  | "["  | "]"
                  | ";"  | ","  | ":"  | "::" | "."  | "@" | "#" | "$" ;
```

---

## 3. Types and Type Expressions

```ebnf
Type              = PrimitiveType
                  | NamedType
                  | PointerType
                  | ReferenceType
                  | ArrayType
                  | TupleType
                  | FunctionType
                  | VectorType
                  | SumType ;

PrimitiveType     = "int" | "float" | "bool" | "string" | "void"
                  | "u8"  | "u16"   | "u32"  | "u64"    | "usize"
                  | "i8"  | "i16"   | "i32"  | "i64"    | "isize"
                  | "f32" | "f64"   | "char" | "rawpointer" ;

NamedType         = [ ModulePath , "::" ] , Identifier , [ GenericTypeArgs ] ;
GenericTypeArgs   = "<" , Type , { "," , Type } , ">" ;

PointerType       = ( "*const" | "*mut" | "*raw" ) , Type ;
ReferenceType     = [ "&" | "&mut" ] , Type ;
ArrayType         = "[" , Type , [ ";" , Expression ] , "]" ;
TupleType         = "(" , [ Type , { "," , Type } ] , ")" ;
FunctionType      = "fn" , "(" , [ Type , { "," , Type } ] , ")" , [ "->" , Type ] ;

VectorType        = "Vec4f" | "Vec8f" | "Vec4i" | "Vec16u8" ;
SumType           = ( "Result" , "<" , Type , "," , Type , ">" )
                  | ( "Option" , "<" , Type , ">" ) ;
```

---

## 4. Program Structure and Declarations

```ebnf
Program           = { TopLevelDeclaration } , EOF ;

TopLevelDeclaration = [ DocComment ] , [ AttributeList ] , 
                    ( ImportDecl
                    | FunctionDecl
                    | ClassDecl
                    | EnumDecl
                    | TraitDecl
                    | ImplDecl
                    | TypeAliasDecl
                    | ExternBlock
                    | GlobalVarDecl
                    | Statement ) ;

AttributeList     = { "#[" , Attribute , "]" } ;
Attribute         = Identifier , [ "(" , [ AttributeArgs ] , ")" ] ;
AttributeArgs     = Expression , { "," , Expression } ;

ImportDecl        = "import" , StringLiteral , [ "as" , Identifier ] , ";" ;

TypeAliasDecl     = "type" , Identifier , [ GenericTypeParams ] , "=" , Type , ";" ;

GlobalVarDecl     = ( "let" | "var" ) , Identifier , [ ":" , Type ] , "=" , Expression , ";" ;
```

### 4.1 Functions and Methods
```ebnf
FunctionDecl      = [ "async" ] , "func" , Identifier , [ GenericTypeParams ] ,
                    "(" , [ ParameterList ] , ")" ,
                    [ "->" , Type ] ,
                    [ WhereClause ] ,
                    ( Block | ";" ) ;

GenericTypeParams = "<" , GenericParam , { "," , GenericParam } , ">" ;
GenericParam      = Identifier , [ ":" , TraitBound ] ;
TraitBound        = NamedType , { "+" , NamedType } ;

WhereClause       = "where" , Type , ":" , TraitBound , { "," , Type , ":" , TraitBound } ;

ParameterList     = Parameter , { "," , Parameter } ;
Parameter         = [ "mut" ] , Identifier , ":" , Type , [ "=" , Expression ] ;
```

### 4.2 Classes and Data Structures
```ebnf
ClassDecl         = "class" , Identifier , [ GenericTypeParams ] ,
                    [ "extends" , NamedType ] ,
                    [ "implements" , NamedType , { "," , NamedType } ] ,
                    "{" , { ClassMember } , "}" ;

ClassMember       = [ DocComment ] , [ AttributeList ] , ( FieldDecl | MethodDecl | ConstructorDecl ) ;
FieldDecl         = [ "pub" ] , Identifier , ":" , Type , [ "=" , Expression ] , ";" ;
ConstructorDecl   = "init" , "(" , [ ParameterList ] , ")" , Block ;
MethodDecl        = FunctionDecl ;
```

### 4.3 Enums and Algebraic Data Types (ADTs)
```ebnf
EnumDecl          = "enum" , Identifier , [ GenericTypeParams ] , "{" ,
                    [ EnumVariantList ] ,
                    "}" ;

EnumVariantList   = EnumVariant , { "," , EnumVariant } , [ "," ] ;
EnumVariant       = [ DocComment ] , Identifier , [ VariantPayload ] , [ "=" , Expression ] ;
VariantPayload    = TuplePayload | StructPayload ;
TuplePayload      = "(" , Type , { "," , Type } , ")" ;
StructPayload     = "{" , FieldDecl , { FieldDecl } , "}" ;
```

### 4.4 Traits and Implementations
```ebnf
TraitDecl         = "trait" , Identifier , [ GenericTypeParams ] , [ ":" , TraitBound ] ,
                    "{" , { TraitItem } , "}" ;

TraitItem         = MethodSignature | AssociatedType ;
MethodSignature   = [ DocComment ] , "func" , Identifier , [ GenericTypeParams ] ,
                    "(" , [ ParameterList ] , ")" , [ "->" , Type ] , ( ";" | Block ) ;
AssociatedType    = "type" , Identifier , [ ":" , TraitBound ] , ";" ;

ImplDecl          = "impl" , [ GenericTypeParams ] , [ NamedType , "for" ] , Type ,
                    [ WhereClause ] ,
                    "{" , { ImplItem } , "}" ;

ImplItem          = FunctionDecl | TypeAliasDecl ;
```

### 4.5 Foreign Function Interface (FFI) & Assembly
```ebnf
ExternBlock       = "extern" , [ StringLiteral ] , "{" , { ExternItem } , "}" ;
ExternItem        = "func" , Identifier , "(" , [ ParameterList ] , ")" , [ "->" , Type ] , ";" ;

InlineAsm         = "asm" , [ "volatile" ] , "{" ,
                    StringLiteral ,
                    [ ":" , AsmOutputs ] ,
                    [ ":" , AsmInputs ] ,
                    [ ":" , AsmClobbers ] ,
                    [ ":" , AsmOptions ] ,
                    "}" ;
```

---

## 5. Statements

```ebnf
Statement         = Block
                  | LetStatement
                  | VarStatement
                  | AssignmentStatement
                  | ExpressionStatement
                  | IfStatement
                  | WhileStatement
                  | ForStatement
                  | MatchStatement
                  | ReturnStatement
                  | BreakStatement
                  | ContinueStatement
                  | DeferStatement
                  | UnsafeStatement
                  | ComptimeStatement
                  | ConcurrencyStatement ;

Block             = "{" , { Statement } , "}" ;

LetStatement      = "let" , Pattern , [ ":" , Type ] , [ "=" , Expression ] , ";" ;
VarStatement      = "var" , Pattern , [ ":" , Type ] , [ "=" , Expression ] , ";" ;

AssignmentStatement = LValue , AssignmentOp , Expression , ";" ;

ExpressionStatement = Expression , ";" ;

IfStatement       = "if" , ( Expression | LetPatternCondition ) , Block ,
                    { "else" , "if" , ( Expression | LetPatternCondition ) , Block } ,
                    [ "else" , Block ] ;

LetPatternCondition = "let" , Pattern , "=" , Expression ;

WhileStatement    = "while" , ( Expression | LetPatternCondition ) , Block ;

ForStatement      = ForInStatement | ForClassicStatement ;
ForInStatement    = "for" , Pattern , "in" , Expression , Block ;
ForClassicStatement = "for" , [ LetStatement | VarStatement | ExpressionStatement ] ,
                            [ Expression ] , ";" ,
                            [ Expression ] , Block ;

MatchStatement    = "match" , Expression , "{" , { MatchArm } , "}" ;
MatchArm          = MatchPattern , [ "if" , Expression ] , "=>" , ( Block | ( Statement ) ) ;
MatchPattern      = Pattern , { "|" , Pattern } ;

ReturnStatement   = "return" , [ Expression ] , ";" ;
BreakStatement    = "break" , [ Expression ] , ";" ;
ContinueStatement = "continue" , ";" ;
DeferStatement    = "defer" , ( Block | ExpressionStatement ) ;
UnsafeStatement   = "unsafe" , Block ;
ComptimeStatement = "comptime" , Block ;
```

### 5.1 Structured Concurrency Statements
```ebnf
ConcurrencyStatement = SpawnStatement | ChannelStatement | NurseryStatement ;

SpawnStatement    = "spawn" , ( Block | CallExpression ) , ";" ;
ChannelStatement  = "channel" , Identifier , [ "<" , Type , ">" ] , [ "(" , Expression , ")" ] , ";" ;
NurseryStatement  = ( "nursery" | "taskgroup" ) , Identifier , Block ;
```

---

## 6. Patterns and Destructuring

```ebnf
Pattern           = WildcardPattern
                  | LiteralPattern
                  | IdentifierPattern
                  | TuplePattern
                  | StructPattern
                  | EnumPattern
                  | ReferencePattern
                  | SlicePattern ;

WildcardPattern   = "_" ;
LiteralPattern    = Literal ;
IdentifierPattern = [ "mut" ] , Identifier , [ "@" , Pattern ] ;
TuplePattern      = "(" , [ Pattern , { "," , Pattern } ] , ")" ;
StructPattern     = NamedType , "{" , [ FieldPattern , { "," , FieldPattern } ] , "}" ;
FieldPattern      = Identifier , [ ":" , Pattern ] ;
EnumPattern       = [ ModulePath , "::" ] , Identifier , "::" , Identifier , [ TuplePattern | StructPattern ] ;
ReferencePattern  = ( "&" | "&mut" ) , Pattern ;
SlicePattern      = "[" , [ Pattern , { "," , Pattern } ] , "]" ;
```

---

## 7. Expressions and Operator Precedence

Precedence (from highest to lowest):

1. **Primary & Postfix**: Literals, identifiers, grouping `()`, field access `.`, call `()`, indexing `[]`, try operator `?`, struct constructor.
2. **Unary**: `!`, `-`, `~`, `*` (deref), `&` / `&mut` (borrow), `await`.
3. **Casting**: `as`.
4. **Multiplicative**: `*`, `/`, `%`.
5. **Additive**: `+`, `-`.
6. **Bit Shift**: `<<`, `>>`.
7. **Relational**: `<`, `<=`, `>`, `>=`.
8. **Equality**: `==`, `!=`.
9. **Bitwise AND**: `&`.
10. **Bitwise XOR**: `^`.
11. **Bitwise OR**: `|`.
12. **Logical AND**: `&&`.
13. **Logical OR**: `||`.
14. **Null Coalescing**: `??`.
15. **Range**: `..`, `..=`.
16. **Closure & Ternary / Control**: `\|x\| => expr`, `if-else` expressions.
17. **Assignment**: `=`, `+=`, `-=`, etc.

```ebnf
Expression        = AssignmentExpr ;

AssignmentExpr    = LogicalOrExpr , [ AssignmentOp , AssignmentExpr ] ;
LogicalOrExpr     = LogicalAndExpr , { "||" , LogicalAndExpr } ;
LogicalAndExpr    = BitwiseOrExpr , { "&&" , BitwiseOrExpr } ;
BitwiseOrExpr     = BitwiseXorExpr , { "|" , BitwiseXorExpr } ;
BitwiseXorExpr    = BitwiseAndExpr , { "^" , BitwiseAndExpr } ;
BitwiseAndExpr    = EqualityExpr , { "&" , EqualityExpr } ;
EqualityExpr      = RelationalExpr , { ( "==" | "!=" ) , RelationalExpr } ;
RelationalExpr    = ShiftExpr , { ( "<" | "<=" | ">" | ">=" ) , ShiftExpr } ;
ShiftExpr         = AdditiveExpr , { ( "<<" | ">>" ) , AdditiveExpr } ;
AdditiveExpr      = MultiplicativeExpr , { ( "+" | "-" ) , MultiplicativeExpr } ;
MultiplicativeExpr= CastExpr , { ( "*" | "/" | "%" ) , CastExpr } ;
CastExpr          = UnaryExpr , { "as" , Type } ;

UnaryExpr         = ( ( "!" | "-" | "~" | "*" | "&" | "&mut" | "await" ) , UnaryExpr )
                  | PostfixExpr ;

PostfixExpr       = PrimaryExpr , { PostfixOp } ;
PostfixOp         = ( "(" , [ ArgumentList ] , ")" )
                  | ( "[" , Expression , "]" )
                  | ( "." , Identifier )
                  | ( "?." , Identifier )
                  | "?" ;

PrimaryExpr       = Literal
                  | Identifier
                  | ( "(" , Expression , ")" )
                  | TupleExpr
                  | ArrayExpr
                  | ClosureExpr
                  | NewStructExpr
                  | MatchExpr
                  | IfExpr
                  | InlineAsm
                  | ComptimeExpr ;

ClosureExpr       = "|" , [ ParameterList ] , "|" , [ "->" , Type ] , ( Block | Expression ) ;
NewStructExpr     = "new" , NamedType , "(" , [ ArgumentList ] , ")" ;
ArgumentList      = Argument , { "," , Argument } ;
Argument          = [ Identifier , ":" ] , Expression ;
```

---

## 8. Conformance & Verifier Matrix

The grammar specified in this document is verified by:
- **Lexer & Parser Implementation**: `compiler/src/lexer.rs`, `compiler/src/parser.rs`.
- **Pure Roze Self-Hosted Parser**: `demos/self_hosted/src/parser.roze`.
- **Semantic Verifier**: `compiler/src/semantic/mod.rs`.
- **Formatting Formatter Engine**: `compiler/src/fmt.rs`.
- **Automated Grammar Conformance Test Suite**: `compiler/tests/golden.rs` and `demos/self_hosted/tests/06_frontend_grammar.roze`.
