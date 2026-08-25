# Syntactic Grammar

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

This chapter is the normative recognition grammar for the source forms whose productions it
defines. The
[Lexical Grammar](13-lexical-grammar.md) owns token formation, comments, and newline tokens. This
chapter owns their composition into lists and source forms. Topical chapters own name resolution, visibility reachability,
typing, ownership, evaluation, and container-specific validity after a source form has been
recognized.

Every production referenced by this chapter is defined here or imported explicitly from the
lexical grammar. A compiler must not infer syntax from examples, names, resolved declarations, or
type information.

## Notation

Productions use this notation:

```text
Name        = one production
"token"     = one exact token spelling
X?          = zero or one X
X*          = zero or more X
X+          = one or more X
X | Y       = X or Y
(X Y)       = grouping
List(X) = newline* (X ("," newline* X)* ","?)? newline*
NonEmptyList(X) = newline* X ("," newline* X)* ","? newline*
LineSequence(X) = newline* (X (newline+ X)*)? newline*
X ~ Y       = X and Y are byte-adjacent tokens
X gap Y     = X and Y are non-joint tokens on the same logical line
```

`Name` means an `identifier` token unless a narrower production says otherwise. `LineSequence`
owns separation for declarations that do not use commas: adjacent elements require at least one
newline, while leading, trailing, and blank lines are accepted. A leaf declaration never consumes
its enclosing separator. Newlines accepted as continuations inside parameter lists, generic lists,
directives, types, requirements, and expressions are not separators in `LineSequence`.

`List` and `NonEmptyList` own newlines immediately after an opening delimiter, after a comma, and
before the closing delimiter. A newline before a required comma is not consumed: the comma stays
at the end of its element's logical line. `List` permits no elements; `NonEmptyList` requires one.

Productions omit a `continuation_newline` pseudo-token for readability. The parser classifies and
consumes exactly the single newlines admitted by the lexical
[Statement Separation](13-lexical-grammar.md#statement-separation) rules before applying
`LineSequence`. Every remaining `newline` is a separator token. This classification uses adjacent
token kinds and delimiter depth only; it does not depend on parsing success, names, or types.

Comments do not change syntactic recognition. Documentation comments are retained as source
metadata and attach according to their lexical kind; removing them leaves the same token grammar.

## Source Files

Every Nocter source uses one grammar root. A package root `index.nct` adds a package-directive
prefix to the same source that defines its root module:

```text
SourceFile = newline*
             (PackageDirectiveSequence newline+)?
             (DependencySequence (newline+ ItemSequence)? | ItemSequence)?
             newline* eof

PackageDirectiveSequence = PackageDirective (newline+ PackageDirective)*
DependencySequence = DependencyDeclaration (newline+ DependencyDeclaration)*
DependencyDeclaration = SeeDeclaration | UseDeclaration
ItemSequence = Item (newline+ Item)*
```

The same grammar applies to single-file programs, package and child-module `index.nct` contracts,
and ordinary module sources. Semantic package rules permit package directives only in the
`index.nct` whose `#package` declares the package. Every top-level `see` and `use` follows the
directive prefix and precedes the first item.

The grammar deliberately does not accept an arbitrary mixture of dependency declarations and
items. A later top-level `see` or `use` is a syntax error even when resolution would otherwise
succeed.

## Package Directives

```text
PackageDirective = "#" PackageDirectiveName ":" DirectiveValue

PackageDirectiveName = "package"
                     | "dependencies"
                     | "lock"
                     | "executable"
                     | "test"

DirectiveValue = StringLiteral
               | integer_literal
               | DirectiveRecord

DirectiveRecord = "{" List(DirectiveField) "}"
DirectiveField  = Name ":" DirectiveValue
```

The closed directive names, allowed repetitions, and record schemas are defined by
[Package Source](15-command-line-interface.md#package-source). The recursive record grammar only
recognizes their common data notation; it does not permit a schema to accept arbitrary fields or
value kinds.

For example:

```nct
#package: {
    name: "example",
    version: "0.1.0",
}
#executable: {
    name: "example",
    module: "./src/app",
}
```

`#target` is not a package directive. It is an item prefix in module source.

## Visibility

```text
Visibility = "pub"
           | "pub" "(" VisibilityScope ")"

VisibilityScope = "." "/"
                | ("." "." "/")+
                | "/"
```

The spellings inside `pub(...)` are closed. A name, package alias, or arbitrary module path is not
a `VisibilityScope`. The visibility chapter determines whether a recognized boundary can be used
from the declaring module and whether an item kind permits visibility at all.

## See and Use Declarations

```text
SeeDeclaration = "see" SeePath

SeePath = RelativeSourcePrefix SourcePathSegment ("/" SourcePathSegment)* "." "nct"
RelativeSourcePrefix = "." "/" | ("." "." "/")+
SourcePathSegment = ModuleSegment

UseDeclaration = Visibility? "use" UseTree

UseTree = ModulePath
        | ModulePath "." ImportSelection
        | "/" "." ImportSelection

ImportSelection = Name ("as" Name)?
                | "{" NonEmptyList(SelectedName) "}"

SelectedName = Name ("as" Name)?

ModulePath = PackageModulePath | PackageAbsoluteModulePath | RelativeModulePath

PackageModulePath  = ModuleSegment ("/" ModuleSegment)*
PackageAbsoluteModulePath = "/" ModuleSegment ("/" ModuleSegment)*
RelativeModulePath = ("." "/" | ("." "." "/")+) ModuleSegment ("/" ModuleSegment)*
```

`ModuleSegment` is the snake-case module segment defined by the lexical grammar. `SeePath`
contains the exact source filename and admits neither a missing `.nct` suffix nor a mixed `./../`
prefix. Semantic resolution permits the canonical target only when both sources have the same
physical module owner. A
`ModulePath` never contains `.nct`; every `use` is a directory-module import.

A block-scope import uses the private `BlockUseDeclaration` form. Its enclosing block owns the
newline separator. Visibility is recognized only on a top-level use declaration. `see` has no
block-scope or visibility-bearing form. Module rules further reject namespace alias re-exports.

## Items

```text
Item = TargetDirective? TargetableItem
     | ConstructDeclaration
     | InstanceDeclaration
     | ConformDeclaration
     | DropDeclaration
     | TestDeclaration

TargetableItem = FunctionDeclaration
               | ConstantDeclaration
               | PrimitiveDeclaration
               | TypeAliasDeclaration
               | StructDeclaration
               | EnumDeclaration
               | InterfaceDeclaration

TargetDirective = "#" "target" ":" StringLiteral newline+
```

```text
ConstantDeclaration = Visibility? "const" Name ":" Type ("=" Expression)?
```

A constant without an initializer is a module-root contract and must join exactly one private
initializer definition. Every other constant declaration has an initializer.

The grammar makes the `#target` attachment structural: it prefixes exactly one targetable item.
It cannot prefix a `use`, `construct`, `instance`, `conform`, `drop`, or `test`, and it cannot occur
without a following targetable item. Target and standard-library authority checks happen after
parsing.

## Shared Declaration Parts

The following productions are reused by declarations and member containers:

```text
GenericParameters = "<" NonEmptyList(Name) ">"

Parameters = "(" List(Parameter) ")"
Parameter  = "..."? Name ":" Type

CallableTail = ":" CallableResult ProvenanceClause? WhereClause?
ProvenanceClause = "from" ProvenanceOrigin ("|" ProvenanceOrigin)*
ProvenanceOrigin = Name

CallableBody = Block?
```

`CallableResult`, `Type`, and `WhereClause` are defined below. A bodyless declaration ends when its
enclosing `LineSequence` reaches a separator or closing brace; later semantic validation permits
it only for a source form that owns an external contract/body split or is intrinsically bodyless.
At the start of `CallableResult`, the identifier spelling `some` commits to `OpaqueResult`; it is
not reconsidered as a `NamedType` if the opaque form is incomplete. This contextual boundary does
not affect value expressions named `some`.

Semantic declaration validation permits at most one `...` parameter in final position on a
supported callable. Sequence-literal definitions require exactly one such parameter and no fixed
parameter. Other declaration forms reject the modifier.

## Functions, Primitives, and Aliases

```text
FunctionDeclaration = Visibility? "func" Name GenericParameters? Parameters
                      CallableTail CallableBody

PrimitiveDeclaration = Visibility? "primitive" Name GenericParameters? Parameters
                       CallableTail

TypeAliasDeclaration = Visibility? "type" Name GenericParameters? "=" Type
                       WhereClause?
```

A primitive never has a source body. An ordinary function is recognized with a body or as a
bodyless contract; module composition and visibility rules decide whether the latter is valid.

## Structs and Enums

```text
StructDeclaration = Visibility? "copy"? "struct" Name GenericParameters? WhereClause?
                    StructBody?

StructBody = "{" LineSequence(StructField) "}"

StructField = Visibility? Name ":" Type

EnumDeclaration = Visibility? "enum" Name GenericParameters? WhereClause?
                  EnumBody?

EnumBody = "{" LineSequence(EnumVariant) "}"

EnumVariant = Name EnumPayload?
EnumPayload = "(" List(Parameter) ")"
```

Struct fields and enum variants are newline-separated declarations. They are not comma-delimited
items. Enum payload parameters are comma-delimited. A bodyless nominal is recognized for a public
opaque contract; module rules require that form in `index.nct` and require one private complete
definition. The grammar recognizes an empty enum body so the enum validity rule can emit its
dedicated “at least one variant” diagnostic; a complete valid enum has one through 256 variants.

`copy` is contextual only immediately before `struct`. It is not a general type modifier and does
not occur on an enum or alias.

## Interfaces

```text
InterfaceDeclaration = Visibility? "interface" Name GenericParameters? WhereClause?
                       "{" LineSequence(InterfaceMember) "}"

InterfaceMember = AssociatedTypeDeclaration
                | PublicInterfaceMethod
                | ImplementationInterfaceMethod

AssociatedTypeDeclaration = "pub" "type" Name InterfaceBounds?
InterfaceBounds = ":" Capability ("+" Capability)*

PublicInterfaceMethod = "pub" "default"? MethodSignature CallableBody
ImplementationInterfaceMethod = "default" MethodSignature Block
```

An interface contract member always writes bare `pub`; it cannot narrow its visibility
independently from the interface. A method without `default` is a bodyless requirement. A method
with `default` is reusable behavior and either carries a block inline or omits it as an eligible
root contract. The matching private interface fragment writes `default method` without visibility
and must carry the body. A block on a method without `default` is invalid.
Fields, operators, coercions, construction entries, drop declarations, and tests have no interface
member production.

## Construction Declarations

```text
ConstructDeclaration = "construct" DeclarationTypePattern
                       ConstructBody

ConstructBody = "{" newline* ConstructMember
                (newline+ ConstructMember)* newline* "}"

ConstructMember = Visibility? ConstructionFunction
                | Visibility? LiteralDeclaration

ConstructionFunction = "func" Name GenericParameters? Parameters
                       CallableTail CallableBody

LiteralDeclaration = "literal" LiteralShape LiteralParameters CallableTail CallableBody

LiteralShape = "[" "]" | EmptyStringLiteral

LiteralParameters = "(" "..." Name ":" Type ")"
                  | "(" Parameter ")"
```

`EmptyStringLiteral` is the joint single-line `string_start string_end` sequence whose source and
decoded contents are both empty (`""`). In this position it denotes a literal shape, not a
decoded runtime string value. Semantic validation pairs the sequence shape with its one element
pack and the string shape with its one ordinary parameter. The parser retains optional
`Visibility`; module-source semantics require it on a public `index.nct` construction contract and
require its omission on the matching private definition. A construction body is nonempty because
a type with no named or literal construction API has no `construct` declaration.

## Instances

```text
InstanceDeclaration = "instance" DeclarationTypePattern WhereClause?
                      "{" LineSequence(InstanceMember) "}"

InstanceMember = InherentMethod
               | CoercionDeclaration
               | EqualityOperator
               | OrderingOperator
               | IndexOperator
               | ExpansionOperator

InherentMethod = Visibility? MethodSignature CallableBody

MethodSignature = "method" Receiver "." Name GenericParameters? Parameters CallableTail
Receiver = "&" "self" | "&+" "self" | "self"

CoercionDeclaration = Visibility? "coerce" BorrowReceiver "as" Type
                      CoercionProvenance? CallableBody
BorrowReceiver = "&" "self" | "&+" "self"
CoercionProvenance = "from" "self"

EqualityOperator = Visibility? "operator" "(" "&" "self" "==" Name ":" "&" "Self" ")"
                   ":" "bool" WhereClause? CallableBody

OrderingOperator = Visibility? "operator" "(" "&" "self" "<" Name ":" "&" "Self" ")"
                   ":" "bool" WhereClause? CallableBody

IndexOperator = Visibility? "operator" "(" IndexReceiver "[" Parameter "]" ")"
                ":" BorrowType ProvenanceClause? WhereClause? CallableBody
IndexReceiver = "&" "self" | "&+" "self"

ExpansionOperator = Visibility? "operator" "(" "..." ExpansionReceiver ")"
                    ":" Type ProvenanceClause? WhereClause? CallableBody
ExpansionReceiver = "&" "self" | "&+" "self" | "self"
```

`BorrowType` is the readonly or readwrite borrowed result form from the type grammar. Fixed
operator syntax is recognized directly rather than parsed as an arbitrary expression signature.
This keeps the declarable operator set closed. `!=`, `>`, `<=`, and `>=` therefore have no
declaration production.

## Explicit Conformances

```text
ConformDeclaration = "conform" DeclarationTypePattern "for" DeclarationTypePattern
                     WhereClause? "{" LineSequence(ConformMember) "}"

ConformMember = AssociatedTypeBinding | ConformMethod

AssociatedTypeBinding = "type" Name "=" Type
ConformMethod = MethodSignature CallableBody
```

Conformance members never write visibility. An `index.nct` conformance contract contains only
associated type bindings; a bodyless `ConformMethod` is invalid because the interface already owns
that signature. A reciprocally seen private conformance definition contains body-bearing
methods and no associated type bindings. The contract and definition repeat the exact conformance
head and form one semantic conformance. An inline conformance in `index.nct` or single-file mode may
contain both bindings and body-bearing methods. Construction entries, fields, operators,
coercions, drop declarations, tests, and extra functions have no conformance-member production.

## Declaration Type Patterns

```text
DeclarationTypePattern = Name PatternArguments?
                       | BuiltinTypePattern
                       | "[" Name "]"

PatternArguments = "<" NonEmptyList(Name) ">"
BuiltinTypePattern = BuiltinScalarType | "str" | "error"
```

Pattern argument slots contain bare binder names only. Concrete and nested types are expressed by
`where` binder refinements, not placed directly in `PatternArguments`. Semantic validation decides
which built-in patterns the active standard-library package may extend and whether the resolved
target kind is legal for `construct`, `instance`, `conform`, or `drop`.

## Drop and Test Declarations

```text
DropDeclaration = "drop" DeclarationTypePattern "(" "&+" "self" ")" Block
TestDeclaration = "test" Name Block
```

Neither declaration accepts visibility, generic parameters before its target/name, an explicit
result type, or a `where` clause. A drop declaration always contains its body. A test body has the
fixed semantic result contract `void!` without writing that contract in source.

## Recognition Boundaries

The following examples are syntax errors, independently of whether their names resolve:

```nct
func late(): void { return }
use std/io.print // imports cannot follow items

pub instance Buffer {} // instance has no declaration visibility

instance Buffer<i32> {} // pattern arguments cannot contain concrete types

instance Buffer<T> {
    operator (&self != other: &Self): bool { return false }
}

#target: "arm64-darwin"
instance Buffer {}
```

The following forms are recognized first and rejected by later semantic validation:

```nct
func missing_body(): i32

enum Empty {}

construct ExternalType { func new(): Self { loop {} } }
```

The first is invalid unless it is an eligible public contract with one matching private definition
connected through reciprocal direct sees;
the second violates the enum variant-count rule; the third violates construction ownership unless
the resolved target belongs to the declaring module or to the authorized standard-library
surface. Keeping those checks outside parsing gives every accepted token sequence one stable
syntax-tree shape.

## Types

```text
Type = PrefixType TypeOutcomeSuffix?

TypeOutcomeSuffix = "?" "!"? | "!"

PrefixType = CallableType | NonCallablePrefix

NonCallablePrefix = "*" PrefixType
                  | "&" NonCallablePrefix
                  | "&+" NonCallablePrefix
                  | TypeAtom

TypeAtom = BuiltinScalarType
         | "str"
         | "error"
         | "void"
         | "never"
         | NamedType
         | SliceType
         | FixedArrayType
         | GroupedType

BuiltinScalarType = "bool"
                  | "i8" | "i16" | "i32" | "i64"
                  | "u8" | "u16" | "u32" | "u64"
                  | "usize" | "isize"

NamedType = NamedTypeHead TypeSelectionSuffix*
NamedTypeHead = Name TypeArguments? | "Self"
TypeArguments = "<" NonEmptyList(Type) ">"
TypeSelectionSuffix = "." Name TypeArguments?

SliceType = "[" Type "]"
FixedArrayType = "[" Type ";" Expression "]"
GroupedType = "(" Type ")"

CallableType = CallableCapability "func" "(" List(CallableParameter) ")"
               ":" Type ProvenanceClause?
CallableCapability = ("&" | "&+")?
CallableParameter = "..."? (Type | Name ":" Type)

BorrowType = "&" NonCallablePrefix | "&+" NonCallablePrefix

CallableResult = Type | OpaqueResult TypeOutcomeSuffix?
OpaqueResult = "some" Name OpaqueArguments?
OpaqueArguments = "<" NonEmptyList(OpaqueArgument) ">"
OpaqueArgument = Type | Name "=" Type
```

Prefix pointer and borrow operators bind before an outcome suffix. Consequently `&T?` is an
optional readonly borrow. A grouped inner type is required for a borrow of an outcome, as in
`&(T?)`.

At one ungrouped layer, `?`, `!`, and `?!` are the recognized suffixes. Reversing the outcome
order uses a grouped inner fallible type, `(T!)?`; `T!?` is not a production. Semantic validation
rejects repeated layers reached through grouping, invalid `void` or `never` positions, unsized
values outside indirection, and opaque results outside their allowed callable-result positions.

`CallableCapability` distinguishes a repeatedly callable readonly value (`&func`), a repeatedly
callable readwrite value (`&+func`), and a once-callable owned value (`func`). Callable parameters
may omit names or use `name: Type`; names exist for provenance clauses but do not change the
structural callable type. One final callable parameter may carry `...`; its pack marker and element
type are part of structural callable identity.

Because `&func` and `&+func` begin callable types, they are not parsed as an ordinary borrow prefix
followed by a separate `func` type. Borrowing a callable type as data requires explicit grouping,
just like borrowing another already composed type.

A dotted named type has one syntax-tree shape. Resolution walks it from left to right: a module
namespace prefix selects one exported type member, while a type prefix selects an associated type.
Thus `parser.Parser<T>` and `T.Item` use the same token-only selection grammar without conflating
their resolved identities. Type arguments are syntactically accepted on a selected segment so a
module-qualified nominal type can be generic; semantic validation rejects arguments on an
associated projection because generic associated types are not supported. A selected type may
then participate in ordinary outer prefix and outcome forms.

After a `Name`, `<` starts `TypeArguments` only when the complete non-empty list and matching `>`
are present in that type position. Otherwise the enclosing expression grammar retains `<` as its
ordering token. This bounded syntactic lookahead never asks whether the name denotes a generic
type. Nested closing `>>` follows the token-subdivision rule below.

The fixed-array length is an `Expression` parsed without name or type information. Semantic
constant evaluation requires it to produce a `usize`; constant generic parameters remain
unsupported.

The lexer emits `>>` as one punctuation token. While parsing type arguments, that token supplies
two consecutive `>` closers only when two currently open type-argument lists require them. Thus
`Outer<Inner<T>>` is canonical and does not require whitespace between the closers. In expression
grammar, the same token remains the right-shift operator; no name or type information participates
in this decision.

The lexer likewise emits `&&` as one token. In a type-prefix position it supplies two readonly
borrow prefixes, so `&&T` is the compact spelling of `&(&T)`. In an expression operator position
it remains logical conjunction.

## Generic Requirements

```text
WhereClause = "where" Predicate ("," Predicate)*

Predicate = CapabilityPredicate
          | CopyPredicate
          | TypeEqualityPredicate
          | OperatorPredicate
          | CoercionPredicate
          | ExpansionPredicate

CapabilityPredicate = Name ":" Capability ("+" Capability)*
CopyPredicate = "copy" Name
TypeEqualityPredicate = Type "=" Type

Capability = NamedType | CallableType

OperatorPredicate = "(" OperatorRequirement ")" ":" Type
OperatorRequirement = "&" Type ("==" | "<") "&" Type
                    | ("&" | "&+") Type "[" Type "]"

CoercionPredicate = ("&" | "&+") Type "as" Type

ExpansionPredicate = "(" "..." ExpansionRequirementSource ")" ":" Type
ExpansionRequirementSource = Name | "&" Name | "&+" Name
```

The same comma-separated predicate grammar applies after functions, methods, aliases, nominal type
declarations, construction members, instances, conformances, and operators wherever their
declaration production includes `WhereClause`. A newline is not a predicate separator.

The parser records the predicate form without proving it. Later validation distinguishes a
general associated-type equality from a declaration-pattern binder refinement, restricts
structural operator operands, checks callable capability multiplicity, and rejects duplicate or
unsatisfied requirements.

An expansion requirement's source is syntactically one binder name with optional readonly or
readwrite capability. Semantic validation requires that name to be a visible generic parameter;
arbitrary constructed source types do not create a second expansion-requirement spelling.

## Blocks and Body Results

```text
Block = "{" newline*
        (BlockUseSequence (newline+ ExecutableSequence)? | ExecutableSequence)?
        newline* "}"

BlockUseSequence = BlockUseDeclaration (newline+ BlockUseDeclaration)*
BlockUseDeclaration = "use" UseTree

ExecutableSequence = Executable (newline+ Executable)*
Executable = Statement | Expression
```

Block imports form one prefix and cannot use visibility. A `use` after the first executable is a
syntax error, even inside a branch whose execution would not reach it.

The final `Expression` in an `ExecutableSequence` is the block's body result. Every earlier
`Expression` is an expression statement. This classification depends only on source position and
does not change after typing; semantic validation requires each non-final expression statement to
have type `void` or `never`. A trailing newline before `}` does not turn a final expression into a
non-final statement.

A block with no final expression has the normal completion type `void` unless reachability proves
that every path terminates. There is no semicolon syntax for suppressing or creating a body result.
`let _ = expression` is the explicit value-discard statement.

## Statements

```text
Statement = BindingStatement
          | AssignmentStatement
          | ReturnStatement
          | BreakStatement
          | ContinueStatement
          | DropStatement
          | WhileStatement
          | LoopStatement
          | ForStatement
          | RegionStatement

BindingStatement = ("let" | "var") BindingTarget TypeAnnotation? "=" Expression
BindingTarget = Name | "_"
TypeAnnotation = ":" Type

AssignmentStatement = AssignmentTarget AssignmentOperator Expression
AssignmentOperator = "=" | "+=" | "-=" | "*=" | "/=" | "%="
AssignmentTarget = PostfixExpression

ReturnStatement = "return" Expression?
BreakStatement = "break"
ContinueStatement = "continue"
DropStatement = "drop" Name

WhileStatement = "while" HeaderExpression Block
LoopStatement = "loop" Block

ForStatement = "for" Name "in" ForSource Block
ForSource = HeaderExpression "..<" HeaderExpression | HeaderExpression

RegionStatement = "region" Name "using" AllocatorPlace Block

NamedPlace = Name ("." Name)*
AllocatorPlace = NamedPlace
```

`PostfixExpression` is the common expression production for a primary followed by field, call,
method, or index suffixes. Recognizing it as an assignment target does not prove that it denotes a
writable place. The place, initialization, borrow, and operator checks happen after parsing. This
keeps `call() = value` structurally an assignment with one invalid target rather than forcing error
recovery to invent another expression tree.

`let _ = expression` is the only discard binding. Later validation rejects `var _`, missing type
information, invalid binding types, assignment to immutable places, assignment to borrowed
parameter bindings, and compound assignment whose target is not definitely initialized.

The `drop` spelling is contextual only at the beginning of a statement followed by one `Name`.
Dropping a field, index, call result, or arbitrary expression has no statement production. An
ordinary call such as `drop(value)` remains an expression using an ordinary name.

`ForSource` gives `..<` a grammar role only in a `for` header. The first alternative is selected
when that token follows the first expression; no other binary range expression exists. A
collection source is one ordinary expression, including an explicit `&`, `&+`, or `move` prefix.
Iteration capability and the rejection of a bare collection are semantic checks.

`AllocatorPlace` is shared by `region ... using` and typed-literal overrides. It admits an existing
binding and statically named fields only. Calls, indexes, literals, conversions, and other
effectful expressions must be evaluated into a binding before selection as an allocation context.

## Control Expressions and Enum Patterns

```text
ControlExpression = IfExpression | MatchExpression

IfExpression = "if" IfCondition Block ElseClause?
IfCondition = HeaderExpression ("is" EnumPattern)?
ElseClause = "else" Block | "else" IfExpression

MatchExpression = "match" HeaderExpression
                  "{" LineSequence(MatchArm) "}"

MatchArm = EnumPattern Block | "_" Block

EnumPattern = Name "." Name EnumPatternPayload?
EnumPatternPayload = "(" List(PayloadSlot) ")"
PayloadSlot = Name | "_"
```

`is` is recognized only between an `if` target expression and its enum pattern. `_` by itself is
recognized only as a `match` fallback arm; inside a payload list it consumes exactly one payload
slot. Nested, literal, named-field, rest, and binding-modifier patterns have no production.

The grammar permits zero match arms and permits a fallback in any source position so the semantic
checker can issue focused exhaustiveness, duplicate-arm, empty-match, and fallback-last
diagnostics. It likewise recognizes any qualified names and payload arity before resolution checks
that they select the target enum and exact variant payload.

An `if`, `if is`, or `match` has one expression node whether it appears as a non-final expression
statement or as a block result. Branch compatibility, missing `else`, exhaustiveness, ownership of
the pattern target, and payload binding types do not affect parsing.

## Expression Precedence

The complete expression grammar, from lowest to highest precedence, is:

```text
Expression = RecoveryExpression

RecoveryExpression = LogicalOrExpression RecoveryClause*
RecoveryClause = "catch" CatchBinding Block
               | "otherwise" Block
CatchBinding = Name | "_"

LogicalOrExpression = LogicalAndExpression ("||" LogicalAndExpression)*
LogicalAndExpression = EqualityExpression ("&&" EqualityExpression)*

EqualityExpression = OrderingExpression (("==" | "!=") OrderingExpression)?
OrderingExpression = ShiftExpression (("<" | "<=" | ">" | ">=") ShiftExpression)?
ShiftExpression = AdditiveExpression (("<<" | ">>") AdditiveExpression)*
AdditiveExpression = MultiplicativeExpression (("+" | "-") MultiplicativeExpression)*
MultiplicativeExpression = ConversionExpression (("*" | "/" | "%") ConversionExpression)*

ConversionExpression = UnaryExpression ("as" Type)*

UnaryExpression = ("!" | "-" | "&" | "&+") UnaryExpression
                | MoveExpression
                | OutcomeExpression

MoveExpression = "move" MovePlace ExpressionOutcomeSuffix?
MovePlace = NamedPlace

OutcomeExpression = PostfixExpression ExpressionOutcomeSuffix?
ExpressionOutcomeSuffix = "?" | "!"

PostfixExpression = PrimaryExpression PostfixSuffix*
PostfixSuffix = CallSuffix | MemberSuffix | IndexSuffix
CallSuffix = "(" List(CallArgument) ")"
CallArgument = Expression | "..." SpreadExpression
MemberSuffix = "." Name
IndexSuffix = "[" Expression "]"
```

Every repeated binary level associates left. `&&` and `||` retain their specified short-circuit
evaluation. Equality and ordering are deliberately non-associative instead: one ungrouped level
accepts at most one comparison. `a < b < c` and `a == b == true` therefore have no production.
When comparing a comparison result is intentional, grouping states that intent explicitly, as in
`(a == b) == true`. The parser does not invent a comparison-chain node.

Unary operators bind more tightly than `as`. Thus `&value as &View` is exactly
`(&value) as &View`, while `&(value as WiderInteger)` requires the authored grouping shown.
Conversion binds more tightly than multiplicative arithmetic. Repeated explicit conversions
associate left; conversion validity and the one-step borrow-coercion rule are semantic checks.

At a unary-prefix position, lexical `&&` supplies two readonly borrow prefixes; after a completed
left operand it remains logical conjunction. Thus `&&value` is parsed as `&(&value)`, while
`left && value` uses `LogicalAndExpression`. No type information selects between them.

One ungrouped layer accepts at most one outcome suffix. `value??`, `value!!`, `value?!`, and
`value!?` have no production. Grouping creates another expression layer, so `(value?)?` is valid
syntax. `move place?` and `move place!` are part of `MoveExpression`: the complete place is moved
before the suffix applies. Calls, indexes, dereferences, and grouped expressions are not
`MovePlace` forms.

Each recovery clause applies to the complete expression to its left. This permits a fallible
optional to handle failure and then absence:

```nct
value catch error { fallback_optional(error) } otherwise { fallback_value() }
```

The blocks make recovery grouping explicit. Nested fallback is written inside the block; there is
no separate precedence guess or implicit flattening. `catch` requires one binding or `_`, while
bare `catch { ... }` has no production.

An index suffix requires its opening `[` to be joint with the preceding expression token.
`values[index]` is indexing; `values [index]` is considered for typed-sequence construction
instead. A call opener may be separated by spaces but must remain on the same logical line as its
callee. `.` may use the lexical continuation rule. These decisions use only token spacing and
newlines.

Assignment and `..<` are absent from this precedence grammar. Assignment is a statement, and
`..<` exists only in a range `for` header.

## Primary Expressions

```text
PrimaryExpression = ControlExpression
                  | ClosureExpression
                  | StructLiteral
                  | TypedLiteral
                  | ArrayLiteral
                  | StringExpression
                  | GenericOwnerMember
                  | ReferenceExpression
                  | integer_literal
                  | byte_literal
                  | "true"
                  | "false"
                  | "none"
                  | GroupedExpression

GroupedExpression = "(" Expression ")"

ReferenceExpression = OwnerHead
GenericOwnerMember = GenericOwnerReference "." Name

GenericOwnerReference = GenericNamedTypeHead TypeSelectionSuffix*
                      | PlainNamedTypeHead PlainTypeSelectionSuffix*
                        GenericTypeSelectionSuffix TypeSelectionSuffix*
GenericNamedTypeHead = Name TypeArguments
PlainNamedTypeHead = Name | "Self"
PlainTypeSelectionSuffix = "." Name
GenericTypeSelectionSuffix = "." Name TypeArguments

OwnerReference = NamedType
               | BuiltinScalarType
               | "str"
               | "error"
OwnerHead = Name | "Self" | BuiltinScalarType | "str" | "error"

StructLiteral = OwnerReference StructInitializer
StructInitializer = "{" List(FieldInitializer) "}"
FieldInitializer = Name ":" Expression

ArrayLiteral = "[" List(Expression) "]"

TypedLiteral = TypedSequenceLiteral | TypedStringLiteral
TypedSequenceLiteral = OwnerReference gap SequenceBody AllocationOverride?
SequenceBody = "[" List(SequenceElement) "]"
SequenceElement = Expression | "..." SpreadExpression
SpreadExpression = Expression

TypedStringLiteral = OwnerReference gap StringLiteral AllocationOverride?
AllocationOverride = "using" AllocatorPlace
```

`OwnerReference` is a syntactic construction owner, not a resolved type. A `Name` without explicit
owner arguments remains the same identifier-shaped primary used for values and types; resolution
decides its namespace. Dotted owner references can select a type from an imported module namespace
or an associated type through the same left-to-right rule as `NamedType`. Explicit
`TypeArguments` in expression position are recognized only in the three productions that
immediately prove construction-owner syntax: before a final `.Name` member, a struct initializer,
or a spaced typed literal. `GenericOwnerReference` spells out the token cases in which at least one
owner segment has explicit arguments. Consequently `left < middle > right` is never reparsed as a
generic owner after name resolution.

Brace-owning expressions are allowed in ordinary expression nesting. At the outer level of a
control header, however, the first `{` at the header's delimiter depth always starts the required
control body:

```text
HeaderExpression = Expression up to but not including the first outer "{"
```

This applies to `if`, `if is`, `while`, `match`, both range endpoints and collection sources in
`for`. At that outer level a `StructLiteral`, `ClosureExpression`, nested `ControlExpression`, or
`RecoveryClause` therefore cannot consume a block. Parentheses, call arguments, index operands,
array elements, and string interpolations enter a nested delimiter depth and restore ordinary
expression grammar:

```nct
if (Flags { ready: true }).ready {
    run()
}
```

Therefore `if Empty {}` always means condition `Empty` followed by an empty body. The parser does
not backtrack based on fields, a second brace, or whether `Empty` resolves as a type.
Likewise, `if (ready) { run() }` is a grouped condition and its `if` body, not a closure used as the
condition. To place a closure itself at the outer header level, group the complete closure:

```nct
if ((value) { value > 0 }) {
    ...
}
```

That example is syntactically unambiguous and then fails semantically unless the closure is valid
as the required header value.

The same grouping rule applies to recovery and nested control expressions:

```nct
if (load_flag() otherwise { false }) {
    run()
}

while (if ready() { keep_running() } else { false }) {
    step()
}
```

Without the outer parentheses, the first block opener belongs to the surrounding control form;
the parser never searches for a later brace that would make another interpretation succeed.

Typed sequence and typed string construction require `gap`: at least one space, tab, or removed
same-line comment separates the owner from `[` or the string opener, and no newline intervenes.
The formatter writes one space. This makes `Vec [1]` construction and `values[1]` indexing distinct
without consulting name resolution. A newline ends the preceding statement because `[` and a
string opener are not continuation leaders.

A fixed array contains only ordinary expressions. Spread is recognized in a typed sequence or a
call argument list whose selected callable has a final argument pack.
`SpreadExpression` has one additional recognition restriction: its first token cannot be `&+`.
When its first token is `move`, it must form the ordinary place-only `MoveExpression`. The accepted
ownership-leading forms are therefore `...&source`, `...move place`, and a bare
`...expression`; mutable `...&+source` has no production.

When a bare `...expression` resolves directly to the current callable's argument-pack parameter,
it is tail forwarding rather than sequence spread. Semantic checking requires it to be the sole
contribution to the selected destination pack; the grammar does not inspect that identity.

`AllocationOverride` belongs only to a typed literal and uses the shared `AllocatorPlace` grammar.
Semantic validation requires that place to carry an established aborting allocator or allocation
context.

## String Expressions

```text
StringExpression = string_start StringPart* string_end
StringPart = string_text
           | interpolation_start Expression interpolation_end

StringLiteral = string_start string_text? string_end
EmptyStringLiteral = single_line_string_start ~ string_end
```

`StringExpression` is the ordinary expression form. With no interpolation part, it is a borrowed
static string literal. With at least one interpolation part, it is the allocating interpolation
expression specified by the string chapter. An interpolation contains exactly one `Expression`;
empty `${}` and multiple newline-separated expressions have no production.

`StringLiteral` is the non-interpolated subset used by package data, `#target`, and typed string
construction. `EmptyStringLiteral` further requires a single-line opener immediately followed by
its joint closer and is used only as the `literal ""` declaration shape. Triple-quoted empty text
does not become a second declaration spelling.

The lexer identifies `interpolation_end` by balanced source delimiters. The parser still owns the
expression between the delimiters, so nested calls, struct literals, control expressions, strings,
and blocks use their ordinary productions without scanning string source a second time.

## Closure Expressions

```text
ClosureExpression = "(" ClosureHead ")" ClosureResult? Block
ClosureHead = newline*
              (ClosureCaptures ";" ClosureParameters? | ClosureParameters)?
              newline*

ClosureCaptures = NonEmptyList(ClosureCapture)
ClosureCapture = "&" Name | "&+" Name | "move" Name

ClosureParameters = NonEmptyList(ClosureParameter)
ClosureParameter = Name TypeAnnotation?

ClosureResult = ":" Type
```

The semicolon is present exactly when explicit captures exist, and the capture segment cannot be
empty. `(&source; value)`, `(&source;)`, `(value)`, and `()` are valid shapes; `(; value)` is not.
Capture and parameter segments independently use the common comma-delimited rule.

Parentheses followed by an optional result annotation and a block select `ClosureExpression`.
Otherwise `(expression)` selects `GroupedExpression`. This token-only boundary makes `(value) {
value }` a closure without treating every parenthesized name as a closure during ordinary
expression parsing.

## Expression Recognition Boundaries

Valid boundary forms include:

```nct
let view = &value as &View
let nested = (move result?)?
let fixed = [1, 2, 3]
let grown = Vec [1, ...source, ...move owned]
let text = String "hello" using arena
let recovered = load() catch _ { fallback() } otherwise { default_value() }
let explicit = (left == right) == expected
```

The following forms are syntax errors rather than alternate spellings:

```nct
let invalid = value??
let invalid = move make_value()
let invalid = Vec [...&+source]
let invalid = (; value) { value }
let invalid = String "value: ${value}"
let invalid = left < middle < right
let invalid = left == middle == right
```

`Vec[index]` remains valid indexing syntax, never typed construction. A newline before `[1, 2, 3]`
ends the preceding expression and begins a separate fixed-array expression; it never becomes a
typed literal across the newline.

Whether a recognized reference is a type, value, construction owner, callable, field, or enum
variant remains a later resolution decision. Whether an operator, coercion, spread, allocation
override, outcome elimination, or closure capture is semantically valid likewise does not alter
the syntax-tree shape.

## Contextual Spellings

The lexer emits the following spellings as identifier-shaped tokens. This grammar gives them a
special role only at the listed boundary:

| Spelling | Contextual grammar position |
| --- | --- |
| `copy` | immediately before `struct`, or immediately after `where` / a predicate comma |
| `where` | the requirement-clause position of an eligible declaration |
| `some` | the start of an opaque callable result |
| `from` | immediately after a callable result type |
| `default` | before an interface default `method` |
| `coerce` | the start of an `instance` member |
| `drop` | the start of a top-level drop declaration or a `drop Name` statement |
| `self` | the fixed receiver position of methods, operators, coercions, and drop declarations |
| `Self` | a type atom or construction-owner head in a type-owned context |
| `error` | a built-in type atom or construction-owner head; otherwise an ordinary value name |
| `_` | a discard binding, catch discard, payload slot, or match fallback |
| `target` | immediately after `#` in an item prefix |
| package directive names | immediately after `#` in a source directive prefix |

Outside those positions, an otherwise valid identifier spelling remains ordinary unless a topical
semantic rule forbids that declaration name. `alloc`, `import`, and `trait` have no contextual
production. Removed `alloc func`, legacy import, and trait declarations are diagnosed without
creating compatibility syntax trees.
