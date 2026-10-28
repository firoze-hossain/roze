# 🌹 The Roze Language Grammar Specification (v1.0.0)

This document provides the formal **Extended Backus-Naur Form (EBNF)** grammar specification for the Roze programming language.

---

## 1. Notation

- `[ ... ]` denotes optional elements (0 or 1 time).
- `{ ... }` denotes repetition (0 or more times).
- `( ... )` denotes grouping.
- `|` denotes alternation (choice).
- `" ... "` denotes terminal tokens or characters.

---

## 2. Lexical Grammar

### 2.1 Whitespace and Comments
```ebnf
Whitespace      = " " | "\t" | "\r" | "\n" ;
LineComment     = "//" { AnyCharExceptNewline } ( "\n" | EOF ) ;
BlockComment    = "/*" { AnyChar | BlockComment } "*/" ;
Comment         = LineComment | BlockComment ;
```

### 2.2 Keywords
```ebnf
Keyword         = "func"     | "let"      | "var"      | "if"       | "else"
                | "while"    | "for"      | "return"   | "class"    | "new"
                | "extern"   | "import"   | "enum"     | "match"    | "trait"
                | "impl"     | "unsafe"   | "null"     | "true"     | "false"
                | "as"       | "in"       | "break"    | "continue" | "type"
                | "where"    | "comptime" | "async"    | "await" ;
```

### 2.3 Identifiers
```ebnf
Identifier      = ( Letter | "_" ) { Letter | Digit | "_" } ;
Letter          = "a" ... "z" | "A" ... "Z" ;
Digit           = "0" ... "9" ;
```

### 2.4 Literals
```ebnf
Literal         = IntegerLiteral | FloatLiteral | StringLiteral | BooleanLiteral | NullLiteral ;

IntegerLiteral  = [ "-" ] Digit { Digit } ;
FloatLiteral    = [ "-" ] Digit { Digit } "." Digit { Digit } ;

StringLiteral   = "\"" { StringChar | EscapeSeq } "\"" ;
StringChar      = AnyCharExceptQuoteOrBackslashOrNewline ;
EscapeSeq       = "\\" ( "n" | "t" | "r" | "\\" | "\"" | "'" | "0" ) ;

BooleanLiteral  = "true" | "false" ;
NullLiteral     = "null" ;
```

### 2.5 Operators and Punctuation
```ebnf
Operator        = "+"  | "-"  | "*"  | "/"  | "%"
                | "==" | "!=" | "<"  | "<=" | ">"  | ">="
                | "&&" | "||" | "!"  | "?"
                | "="  | "+=" | "-=" | "*=" | "/=" | "%="
                | "->" | "=>" | ".." | "..=" ;

Punctuation     = "(" | ")" | "{" | "}" | "[" | "]"
                | ";" | "," | ":" | "." | "::" ;
```

---

## 3. Syntactic Grammar

### 3.1 Program and Top-Level Declarations
```ebnf
Program         = { TopLevelItem } EOF ;

TopLevelItem    = ImportDecl
                | FunctionDecl
                | ClassDecl
                | EnumDecl
                | TraitDecl
                | ImplDecl
                | ExternDecl
                | Statement ;

ImportDecl      = "import" StringLiteral [ ";" ] ;

FunctionDecl    = [ "async" ] "func" Identifier [ TypeParams ] "(" [ ParameterList ] ")" [ "->" Type ] [ WhereClause ] Block ;

ParameterList   = Parameter { "," Parameter } ;
Parameter       = Identifier ":" Type ;

TypeParams      = "<" TypeParam { "," TypeParam } ">" ;
TypeParam       = Identifier [ ":" TraitBoundList ] ;
TraitBoundList  = Identifier { "+" Identifier } ;

WhereClause     = "where" WherePredicate { "," WherePredicate } ;
WherePredicate  = Identifier ":" TraitBoundList ;
```

### 3.2 Types
```ebnf
Type            = PrimitiveType
                | NamedType
                | GenericType
                | FunctionType ;

PrimitiveType   = "int" | "float" | "bool" | "string" | "void" | "u8" | "rawpointer" ;
NamedType       = Identifier ;
GenericType     = Identifier "<" Type { "," Type } ">" ;
FunctionType    = "(" [ TypeList ] ")" "->" Type ;
TypeList        = Type { "," Type } ;
```

### 3.3 Class Declarations
```ebnf
ClassDecl       = "class" Identifier [ TypeParams ] "{" { FieldDecl [ "," ] } "}" ;
FieldDecl       = Identifier ":" Type ;
```

### 3.4 Enum Declarations
```ebnf
EnumDecl        = "enum" Identifier [ TypeParams ] "{" { EnumVariant [ "," ] } "}" ;
EnumVariant     = Identifier [ "(" ParameterList ")" ] ;
```

### 3.5 Trait & Implementation Declarations
```ebnf
TraitDecl          = "trait" Identifier [ TypeParams ] "{" { TraitItem } "}" ;
TraitItem          = TraitMethodDecl | AssociatedTypeDecl ;
TraitMethodDecl    = "func" Identifier "(" [ ParameterList ] ")" [ "->" Type ] ";" ;
AssociatedTypeDecl = "type" Identifier ";" ;

ImplDecl           = "impl" [ TypeParams ] Identifier "for" Type "{" { ImplItem } "}" ;
ImplItem           = FunctionDecl | AssociatedTypeDef ;
AssociatedTypeDef  = "type" Identifier "=" Type ";" ;
```

### 3.6 Extern Declarations
```ebnf
ExternDecl      = "extern" StringLiteral "func" Identifier "(" [ ParameterList ] ")" [ "->" Type ] ";" ;
```

---

## 4. Statements

```ebnf
Statement       = Block
                | LetStatement
                | AssignStatement
                | IfStatement
                | WhileStatement
                | ForStatement
                | MatchStatement
                | ReturnStatement
                | UnsafeStatement
                | ComptimeStatement
                | ExpressionStatement ;

Block           = "{" { Statement } "}" ;

LetStatement    = ( "let" | "var" ) Identifier [ ":" Type ] [ "=" Expression ] ";" ;

AssignStatement = LValue ( "=" | "+=" | "-=" | "*=" | "/=" | "%=" ) Expression ";" ;
LValue          = Identifier { MemberAccess | IndexAccess } ;

IfStatement     = "if" Expression Block [ "else" ( IfStatement | Block ) ] ;

WhileStatement  = "while" Expression Block ;

ForStatement    = "for" [ Statement ] ";" [ Expression ] ";" [ Expression ] Block ;

MatchStatement  = "match" Expression "{" { MatchArm } "}" ;
MatchArm        = Pattern [ "if" Expression ] "=>" ( Block | Statement | Expression [ ";" ] ) ;
Pattern         = RangePattern
                | EnumPattern
                | StructPattern
                | TuplePattern
                | SlicePattern
                | Literal
                | Identifier
                | "_" ;

RangePattern    = Expression ( ".." | "..=" ) Expression ;
EnumPattern     = [ Identifier "::" ] Identifier [ "(" [ PatternList ] ")" ] ;
StructPattern   = Identifier "{" [ FieldPatternList ] "}" ;
FieldPatternList = FieldPattern { "," FieldPattern } ;
FieldPattern    = Identifier [ ":" Pattern ] ;
TuplePattern    = "(" [ PatternList ] ")" ;
SlicePattern    = "[" [ SlicePatternElements ] "]" ;
SlicePatternElements = PatternList [ "," ".." [ Identifier ] [ "," PatternList ] ]
                     | ".." [ Identifier ] [ "," PatternList ] ;
PatternList     = Pattern { "," Pattern } ;

ReturnStatement = "return" [ Expression ] ";" ;

UnsafeStatement = "unsafe" Block ;

ComptimeStatement = "comptime" ( Block | Statement | Expression [ ";" ] ) ;

ExpressionStatement = Expression ";" ;
```

---

## 5. Expressions & Precedence

Precedence levels are ordered from lowest (1) to highest (12):

```ebnf
Expression      = LogicalOr ;

LogicalOr       = LogicalAnd { "||" LogicalAnd } ;                 (* Level 1 *)
LogicalAnd      = Equality { "&&" Equality } ;                     (* Level 2 *)
Equality        = Relational { ( "==" | "!=" ) Relational } ;      (* Level 3 *)
Relational      = Bitwise { ( "<" | "<=" | ">" | ">=" ) Bitwise } ;(* Level 4 *)
Bitwise         = Additive { ( "&" | "|" | "^" ) Additive } ;      (* Level 5 *)
Additive        = Multiplicative { ( "+" | "-" ) Multiplicative } ;(* Level 6 *)
Multiplicative  = Unary { ( "*" | "/" | "%" ) Unary } ;            (* Level 7 *)

Unary           = ( "-" | "!" | "*" | "&" | "await" ) Unary          (* Level 8 *)
                | TryExpression ;

TryExpression   = Postfix [ "?" ] ;                                (* Level 9 *)

Postfix         = Primary { MemberAccess | IndexAccess | Call | MethodCall } ; (* Level 10 *)

MemberAccess    = "." Identifier ;
IndexAccess     = "[" Expression "]" ;
Call            = "(" [ ArgumentList ] ")" ;
MethodCall      = "." Identifier "(" [ ArgumentList ] ")" ;
ArgumentList    = Expression { "," Expression } ;

Primary         = Literal                                          (* Level 11 *)
                | Identifier
                | NewExpression
                | ClosureExpression
                | ComptimeExpression
                | ListLiteral
                | "(" Expression ")" ;

NewExpression   = "new" Identifier [ TypeArgs ] "(" [ ArgumentList ] ")" ;
TypeArgs        = "<" Type { "," Type } ">" ;

ClosureExpression = "(" [ ParameterList ] ")" [ "->" Type ] "=>" ( Block | Expression ) ;

ComptimeExpression = "comptime" ( Block | Expression ) ;

ListLiteral     = "[" [ ArgumentList ] "]" ;
```

---

## 6. End of Grammar Specification
