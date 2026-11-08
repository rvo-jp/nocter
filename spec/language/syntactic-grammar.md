# Syntactic Grammar

This chapter is the normative recognition grammar for the source forms whose productions it
defines. The
[Lexical Grammar](lexical-grammar.md) owns token formation, comments, and newline tokens. This
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
[Statement Separation](lexical-grammar.md#statement-separation) rules before applying
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
                     | "executable"
                     | "test"

DirectiveValue = StringLiteral
               | integer_literal
               | DirectiveRecord

DirectiveRecord = "{" List(DirectiveField) "}"
DirectiveField  = Name ":" DirectiveValue
```

The closed directive names, allowed repetitions, and record schemas are defined by
[Packages and Package Source](packages.md#directive-data). The recursive record grammar only
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

UseTree = ModulePath ("as" Name)?
        | "/" "as" Name
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
Semantic validation permits `as` on a private namespace import, rejects it on a namespace
re-export, and permits `ImportSelection` only for exported type-namespace declarations. A function,
constant, or other value is reached through an imported module namespace.

A block-scope import uses the private `BlockUseDeclaration` form. Its enclosing block owns the
newline separator. Visibility is recognized only on a top-level use declaration. `see` has no
block-scope or visibility-bearing form. Module rules further reject namespace alias re-exports.

## Items

```text
Item = TargetDirective? TargetableItem
     | ConstructDeclaration
     | InstanceDeclaration
     | DropDeclaration
     | TestDeclaration

TargetableItem = FunctionDeclaration
               | ConstantDeclaration
               | StaticDeclaration
               | PrimitiveTypeDeclaration
               | TypeAliasDeclaration
               | StructDeclaration
               | EnumDeclaration
               | InterfaceDeclaration

TargetDirective = "#" "target" ":" StringLiteral newline+
```

```text
ConstantDeclaration = Visibility? "const" Name ":" Type ("=" Expression)?
StaticDeclaration = Visibility? "static" Name ":" Type ("=" StaticExpression)?
```

A constant's `Name` must satisfy the semantic `UPPER_SNAKE_CASE` rule in
[Compile-Time Constants](constants.md#declaration). The lexer keeps one identifier token class;
the parser does not duplicate the naming-policy check.

A constant without an initializer is a module-root contract and must join exactly one private
initializer definition. Every other constant declaration has an initializer.

A static without an initializer is likewise a module-root contract and must join exactly one
private definition. `StaticExpression` is the recursively frozen initializer language defined by
[Compile-Time Constants](constants.md#immutable-static-data); it is deliberately
narrower than an ordinary runtime expression.

The grammar makes the `#target` attachment structural: it prefixes exactly one targetable item.
It cannot prefix a `use`, `construct`, `instance`, `drop`, or `test`, and it cannot occur
without a following targetable item. Target and standard-library authority checks happen after
parsing.

## Shared Declaration Parts

The following productions are reused by declarations and member containers:

```text
GenericParameters = "<" NonEmptyList(GenericParameter) ">"
GenericParameter = Name | "const" Name ":" Type

Parameters = "(" List(ParameterContract) ")"
ParameterContract = Parameter ProvenanceClause?
Parameter = "..."? Name ":" Type

CallableTail = ":" CallableResult ProvenanceClause? WhereClause?
ProvenanceClause = "from" ProvenanceOrigin ("|" ProvenanceOrigin)*
ProvenanceOrigin = Name | "self" | "static"

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

A `ProvenanceClause` following a parameter or type annotation qualifies that complete value. A
receiver with a clause uses the parenthesized form so the clause cannot be confused with the method
selection dot. Header origin names resolve against the complete receiver-and-parameter set rather
than only earlier source positions. Local annotation origins resolve through ordinary lexical
visibility. The semantic rules are defined by
[Memory, Regions, and Allocators](memory-and-regions.md#value-storage-contracts).

## Functions, Primitive Types, and Aliases

```text
FunctionDeclaration = Visibility? CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? AsyncModifier? "func" Name GenericParameters? Parameters
                      CallableTail CallableBody
                    | Visibility? CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? "primitive" "func" Name GenericParameters? Parameters
                      CallableTail

CompileTimeModifier = "const"
NoAllocModifier = "noalloc"
NoTrapModifier = "notrap"
BlockingModifier = "blocking"
AsyncModifier = "async"

PrimitiveTypeDeclaration = Visibility? "primitive" "type" PrimitiveTypeName

PrimitiveTypeName = Name | "void" | "never"

TypeAliasDeclaration = Visibility? "type" Name GenericParameters? "=" Type
                       WhereClause?
```

A function carrying `primitive` never has a source body. A function without that modifier is
recognized with a body or as a bodyless contract; module composition and visibility rules decide
whether the latter is valid. Semantic toolchain validation restricts `PrimitiveTypeName` to the
exact closed built-in declaration selected for that source token; accepting a syntactic `Name`
here does not create an open user-defined primitive-type facility.

`CompileTimeModifier` promises that the same ordinary callable implementation is available to the
compile-time evaluator. Its canonical position follows visibility and precedes `noalloc`.
`NoTrapModifier` promises that immediate execution cannot reach a source-semantic safety trap. Its
canonical position follows `noalloc` and precedes `blocking`. `blocking` admits synchronous waiting
by an immediate function or primitive function. Its canonical position follows `notrap` and
precedes `async` or `primitive`. An unqualified public function does
not admit synchronous waiting. `async` is admitted only on an ordinary function. Its canonical
position follows `blocking` and precedes `func`; it has no primitive-function production.
Combining `async` with `const`, `noalloc`, `notrap`, or `blocking` is a semantic error. A primitive function
cannot promise `const` because it has no checked source body for the compile-time evaluator. The
`async` modifier declares deferred execution and is independent of the callable's result type.

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
InterfaceBounds = "impl" InterfaceApplication ("+" InterfaceApplication)*

PublicInterfaceMethod = "pub" CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? AsyncModifier? "default"? MethodSignature CallableBody
ImplementationInterfaceMethod = CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? AsyncModifier? "default" MethodSignature Block
```

An interface contract member always writes bare `pub`; it cannot narrow its visibility
independently from the interface. A method without `default` is a bodyless requirement. A method
with `default` is reusable behavior and either carries a block inline or omits it as an eligible
root contract. The matching private interface fragment writes `default method` without visibility
and must carry the body. A block on a method without `default` is invalid.
`noalloc`, `notrap`, `blocking`, and `async` have the same canonical positions and meanings as on a
function. An interface requirement and its private contract body must agree on every authored
guarantee and execution modifier. A nonblocking
inherent method may satisfy a `blocking` interface requirement, but a blocking method cannot
satisfy an unqualified requirement. Combining `async` with `const`, `noalloc`, `notrap`, or `blocking` is
invalid.
Fields, operators, coercions, construction entries, drop declarations, and tests have no interface
member production.

## Construction Declarations

```text
ConstructDeclaration = "construct" DeclarationTypePattern
                       ConstructBody

ConstructBody = "{" newline* ConstructMember
                (newline+ ConstructMember)* newline* "}"

ConstructMember = Visibility? CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? AsyncModifier? ConstructionFunction
                | Visibility? NoAllocModifier? NoTrapModifier? LiteralDeclaration

ConstructionFunction = "func" Name GenericParameters? Parameters
                       CallableTail CallableBody

LiteralDeclaration = "literal" LiteralShape LiteralParameters CallableTail CallableBody

LiteralShape = "[" "]" | EmptyStringLiteral

LiteralParameters = "(" "..." Name ":" Type ProvenanceClause? ")"
                  | "(" ParameterContract ")"
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
               | InterfaceImplementation
               | CoercionDeclaration
               | EqualityOperator
               | OrderingOperator
               | IndexOperator
               | ExpansionOperator

InherentMethod = Visibility? CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? AsyncModifier? MethodSignature CallableBody

InterfaceImplementation = "impl" InterfaceApplication

InterfaceApplication = NamedType AssociatedBindings?
AssociatedBindings = "{" AssociatedTypeBinding
                     ("," AssociatedTypeBinding)* ","? "}"
AssociatedTypeBinding = "." Name "=" Type

MethodSignature = "method" Receiver "." Name GenericParameters? Parameters CallableTail
Receiver = SimpleReceiver | "(" SimpleReceiver ProvenanceClause ")"
SimpleReceiver = "&" "self" | "&+" "self" | "self"

An inherent method follows the same `blocking`, `async`, `noalloc`, and `notrap` rules as a function.
Literals, coercions, operators, expansion, and destruction have no `blocking` production and must
remain nonblocking because they may be invoked implicitly. They may publish `noalloc` and `notrap`
when their implementations satisfy those independent guarantees.

CoercionDeclaration = Visibility? NoAllocModifier? NoTrapModifier? "coerce" BorrowReceiver "as" Type
                      CoercionProvenance? CallableBody
BorrowReceiver = "&" "self" | "&+" "self"
CoercionProvenance = "from" "self"

EqualityOperator = Visibility? NoAllocModifier? NoTrapModifier? "operator" "(" "&" "self" "==" Name ":" "&" "Self" ")"
                   ":" "bool" WhereClause? CallableBody

OrderingOperator = Visibility? NoAllocModifier? NoTrapModifier? "operator" "(" "&" "self" "<" Name ":" "&" "Self" ")"
                   ":" "bool" WhereClause? CallableBody

IndexOperator = Visibility? NoAllocModifier? NoTrapModifier? "operator" "(" IndexReceiver "[" ParameterContract "]" ")"
                ":" BorrowType ProvenanceClause? WhereClause? CallableBody
IndexReceiver = "&" "self" | "&+" "self"

ExpansionOperator = Visibility? NoAllocModifier? NoTrapModifier? "operator" "(" "..." ExpansionReceiver ")"
                    ":" Type ProvenanceClause? WhereClause? CallableBody
ExpansionReceiver = "&" "self" | "&+" "self" | "self"
```

`BorrowType` is the readonly or readwrite borrowed result form from the type grammar. Fixed
operator syntax is recognized directly rather than parsed as an arbitrary expression signature.
This keeps the declarable operator set closed. `!=`, `>`, `<=`, and `>=` therefore have no
declaration production.

## Instance Interface Implementations

An `InterfaceImplementation` is a bodyless instance member. It contains one nominal interface
application and all associated bindings for that application. The containing instance supplies the
target pattern and conditional requirements. Required method implementations are ordinary
`InherentMethod` declarations and are matched semantically after all instance fragments have been
collected. An implementation member never writes visibility and cannot contain callable bodies,
operators, coercions, construction entries, fields, drop declarations, or tests.

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
target kind is legal for `construct`, `instance`, or `drop`.

## Drop and Test Declarations

```text
DropDeclaration = NoAllocModifier? NoTrapModifier? "drop" DeclarationTypePattern "(" "&+" "self" ")" Block
TestDeclaration = "test" Name Block
```

Neither declaration accepts visibility, generic parameters before its target/name, an explicit
result type, or a `where` clause. A drop declaration always contains its body. A test body has the
fixed semantic result contract `void!` without writing that contract in source.

## Recognition Boundaries

The following examples are syntax errors, independently of whether their names resolve:

```nct
func late(): void { return }
use std/io // imports cannot follow items

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

PrefixType = ErasedCallableType | CallableType | NonCallablePrefix

NonCallablePrefix = "*" PrefixType
                  | "&" NonCallablePrefix
                  | "&+" NonCallablePrefix
                  | FutureType
                  | TypeAtom

FutureType = "future" Type

TypeAtom = BuiltinScalarType
         | "str"
         | "error"
         | "void"
         | "never"
         | NamedType
         | SliceType
         | FixedArrayType
         | TupleType
         | GroupedType

BuiltinScalarType = "bool"
                  | "f32" | "f64"
                  | "i8" | "i16" | "i32" | "i64"
                  | "u8" | "u16" | "u32" | "u64"
                  | "usize" | "isize"

NamedType = NamedTypeHead TypeSelectionSuffix*
NamedTypeHead = Name GenericArguments? | "Self"
GenericArguments = "<" NonEmptyList(GenericArgument) ">"
GenericArgument = Type | StructuralConstantExpression
TypeSelectionSuffix = "." Name GenericArguments?

SliceType = "[" Type "]"
FixedArrayType = "[" Type ";" Expression "]"
GroupedType = "(" Type ")"
TupleType = "(" Type "," Type ("," Type)* ","? ")"

CallableType = CompileTimeModifier? NoAllocModifier? NoTrapModifier? BlockingModifier? CallableCapability "func" "(" List(CallableParameter) ")"
               ":" Type ProvenanceClause?
ErasedCallableType = "any" CallableType
CallableCapability = ("&" | "&+")?
CallableParameter = "..."? (Type | Name ":" Type) ProvenanceClause?

BorrowType = "&" NonCallablePrefix | "&+" NonCallablePrefix

CallableResult = Type | OpaqueResult TypeOutcomeSuffix?
OpaqueResult = "some" InterfaceApplication
```

`StructuralConstantExpression` is the expression subset defined by
[Compile-Time Constants](constants.md#structural-constant-arguments). A generic argument syntax
node retains its parsed type or expression structure without consulting a declaration. A bare name
is the common leaf of both forms; semantic lowering classifies that leaf once from the selected
declaration's parameter schema. It is not reparsed from source text.

Prefix pointer and borrow operators bind before an outcome suffix. Consequently `&T?` is an
optional readonly borrow. A grouped inner type is required for a borrow of an outcome, as in
`&(T?)`.

`future` consumes a complete `Type` operand rather than a `NonCallablePrefix`. Consequently
`future T!` is `future (T!)`, while `(future T)!` puts the fallible layer outside the asynchronous
computation. This distinction is syntactic and does not depend on name or type resolution.

At one ungrouped layer, `?`, `!`, and `?!` are the recognized suffixes. Reversing the outcome
order uses a grouped inner fallible type, `(T!)?`; `T!?` is not a production. Semantic validation
rejects repeated layers reached through grouping, invalid `void` or `never` positions, unsized
values outside indirection, and opaque results outside their allowed callable-result positions.

`CallableCapability` distinguishes a repeatedly callable readonly value (`&func`), a repeatedly
callable readwrite value (`&+func`), and a once-callable owned value (`func`). Callable parameters
may omit names or use `name: Type`; names exist for provenance clauses but do not change the
structural callable type. One final callable parameter may carry `...`; its pack marker and element
type are part of structural callable identity.

Because `&func` and `&+func` begin callable annotations, they are not parsed as an ordinary borrow
prefix followed by a separate `func` type. Their leading capability describes invocation access.
An unprefixed callable type remains statically witnessed. Prefixing the complete callable type with
`any` selects its distinct sized erased representation; `any` cannot prefix another type form.

`CompileTimeModifier` is accepted on function declarations, construction functions, methods, and
callable types. `NoAllocModifier` and `NoTrapModifier` are accepted only on callable declarations,
drop declarations, and callable types. Their canonical positions follow visibility in
`const noalloc notrap` order and
precede `blocking`, `async`, `primitive`, `default`, the callable kind, or the callable capability.
Repeating a modifier or placing one on a nominal type, constant, interface, instance, test,
parameter, or ordinary type has no production. Construction literals, coercions, operators,
expansion, and destruction do not accept `CompileTimeModifier`.

A dotted named type has one syntax-tree shape. Resolution walks it from left to right: a module
namespace prefix selects one exported type member, while a type prefix selects an associated type.
Thus `parser.Parser<T>` and `T.Item` use the same token-only selection grammar without conflating
their resolved identities. Type arguments are syntactically accepted on a selected segment so a
module-qualified nominal type can be generic; semantic validation rejects arguments on an
associated projection because generic associated types are not supported. A selected type may
then participate in ordinary outer prefix and outcome forms.

After a `Name`, `<` starts `GenericArguments` only when the complete non-empty list and matching `>`
are present in that type position. Otherwise the enclosing expression grammar retains `<` as its
ordering token. This bounded syntactic lookahead never asks whether the name denotes a generic
type. Nested closing `>>` follows the token-subdivision rule below.

The fixed-array length is an `Expression` parsed without name or type information. Semantic
constant evaluation requires it to produce a `usize`; a visible `usize` constant parameter is a
valid structural value.

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

Predicate = InterfacePredicate
          | CallablePredicate
          | CopyPredicate
          | TypeEqualityPredicate
          | OperatorPredicate
          | CoercionPredicate
          | ExpansionPredicate

InterfacePredicate = RequirementSubject "impl" InterfaceApplication
RequirementSubject = Name ("." Name)* | "Self"
CallablePredicate = Name ":" Type
CopyPredicate = "copy" Name
TypeEqualityPredicate = Type "=" Type

StructuralGuarantees = NoAllocModifier? NoTrapModifier?

OperatorPredicate = StructuralGuarantees "(" OperatorRequirement ")" ":" Type
OperatorRequirement = "&" Type ("==" | "<") "&" Type
                    | ("&" | "&+") Type "[" Type "]"

CoercionPredicate = StructuralGuarantees ("&" | "&+") Type "as" Type

ExpansionPredicate = StructuralGuarantees "(" "..." ExpansionRequirementSource ")" ":" Type
ExpansionRequirementSource = Name | "&" Name | "&+" Name
                           | "Self" | "&" "Self" | "&+" "Self"
```

The same comma-separated predicate grammar applies after functions, methods, aliases, nominal type
declarations, construction members, instances, and operators wherever their
declaration production includes `WhereClause`. A newline is not a predicate separator.

The parser records the predicate form without proving it. Later validation distinguishes a
declaration-pattern binder refinement from other predicates, restricts structural operator
operands, and rejects duplicate or unsatisfied requirements. A type equality is accepted only as a
directed binder refinement belonging to a declaration type pattern; associated projection equality
is expressed by `AssociatedBindings` on the relevant interface application.

`StructuralGuarantees` applies only to operator, coercion, and expansion predicates. It uses the
same canonical `noalloc notrap` order as immediate callable declarations. `const`, `blocking`, and
`async` are not structural-operation requirement prefixes. An omitted prefix makes no allocation-
or trap-freedom promise. A stronger proven operation may satisfy a weaker requirement, but the
reverse is invalid.

`Self` is accepted as a requirement subject only in an interface header. It may be the subject of
an interface predicate or the operand/source of equality, strict ordering, indexing, borrow
coercion, or expansion. Callable, `copy`, and type-equality predicates do not accept `Self`.
Semantic validation substitutes the complete interface application before checking these
prerequisites; the grammar does not infer a dependency from member names.

`impl` introduces only a nominal interface requirement. The colon in `CallablePredicate` is the
ordinary type-annotation separator and its right operand must normalize to a structural callable
type; this permits a callable type alias but not a nominal interface. Conversely, `func(...)` is
not an interface application and cannot follow `impl`.

An expansion requirement's source is syntactically one binder name or contextual `Self` with
optional readonly or readwrite capability. Semantic validation requires a visible generic
parameter outside an interface header; arbitrary constructed source types do not create a second
expansion-requirement spelling.

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
          | ForAwaitStatement
          | RegionStatement

BindingStatement = ("let" | "var") BindingPattern TypeAnnotation? "=" Expression
BindingPattern = Name | "_" | "(" BindingPattern "," BindingPattern ("," BindingPattern)* ","? ")"
TypeAnnotation = ":" Type ProvenanceClause?

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
ForAwaitStatement = "for" "await" Name "in" HeaderExpression Block

RegionStatement = "region" Name "using" AllocatorPlace Block

NamedPlace = Name ("." (Name | integer_literal))*
AllocatorPlace = NamedPlace
```

`PostfixExpression` is the common expression production for a primary followed by field, call,
method, or index suffixes. Recognizing it as an assignment target does not prove that it denotes a
writable place. The place, initialization, borrow, and operator checks happen after parsing. This
keeps `call() = value` structurally an assignment with one invalid target rather than forcing error
recovery to invent another expression tree.

`_` discards one leaf of a binding. A complete `_` binding remains valid only with `let`; tuple
patterns may contain discard leaves under either `let` or `var`. Later validation rejects invalid
tuple shape or arity, missing type information, invalid binding types, assignment to immutable
places, assignment to borrowed parameter bindings, and compound assignment whose target is not
definitely initialized.

The `drop` spelling is contextual only at the beginning of a statement followed by one `Name`.
Dropping a field, index, call result, or arbitrary expression has no statement production. An
ordinary call such as `drop(value)` remains an expression using an ordinary name.

`ForSource` gives `..<` a grammar role only in a `for` header. The first alternative is selected
when that token follows the first expression; no other binary range expression exists. A
collection source is one ordinary expression, including an explicit `&`, `&+`, or `move` prefix.
Iteration capability and the rejection of a bare collection are semantic checks.

`AllocatorPlace` is shared by `region ... using` and typed-literal overrides. It admits an existing
binding followed by statically selected named fields or tuple positions. Calls, indexes, literals,
conversions, and other effectful expressions must be evaluated into a binding before selection as
an allocation context.

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

OutcomeExpression = AwaitOperand ExpressionOutcomeSuffix?
AwaitOperand = "await" AwaitOperand | PostfixExpression
ExpressionOutcomeSuffix = "?" | "!"

PostfixExpression = PrimaryExpression PostfixSuffix*
PostfixSuffix = CallSuffix | MemberSuffix | TupleElementSuffix | IndexSuffix
CallSuffix = "(" List(CallArgument) ")"
CallArgument = Expression | "..." SpreadExpression
MemberSuffix = "." Name
TupleElementSuffix = "." integer_literal
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

`await` binds to the complete postfix expression immediately following it, and an outcome suffix
then applies to the awaited result. Thus `await operation()?` propagates failure from the value
produced by awaiting `operation()`. Repeated `await` prefixes retain one syntax node per consumed
computation. Suspension legality and operand ownership are semantic checks.

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
                  | TupleExpression
                  | GroupedExpression

GroupedExpression = "(" Expression ")"
TupleExpression = "(" Expression "," Expression ("," Expression)* ","? ")"

ReferenceExpression = OwnerHead
GenericOwnerMember = GenericOwnerReference "." Name

GenericOwnerReference = GenericNamedTypeHead TypeSelectionSuffix*
                      | PlainNamedTypeHead PlainTypeSelectionSuffix*
                        GenericTypeSelectionSuffix TypeSelectionSuffix*
GenericNamedTypeHead = Name GenericArguments
PlainNamedTypeHead = Name | "Self"
PlainTypeSelectionSuffix = "." Name
GenericTypeSelectionSuffix = "." Name GenericArguments

OwnerReference = NamedType
               | BuiltinScalarType
               | "str"
               | "error"
OwnerHead = Name | "Self" | BuiltinScalarType | "str" | "error"

StructLiteral = OwnerReference StructInitializer
StructInitializer = "{" List(FieldInitializer) "}"
FieldInitializer = Name ":" Expression

ArrayLiteral = "[" (ArrayRepeat | List(Expression)) "]"
ArrayRepeat = Expression ";" Expression

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
`GenericArguments` in expression position are recognized only in the three productions that
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

A fixed array contains only ordinary expressions. A semicolon after its first expression selects
the repeat form; commas select the enumerated form. Spread is recognized in a typed sequence or a
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
| `read` | Future Direction v2.0.0: scoped read binding only |
| `edit` | Future Direction v2.0.0: scoped update binding or input operation |
| `take` | Future Direction v2.0.0: value-receiving input operation |
| `copy` | immediately before `struct`, or immediately after `where` / a predicate comma |
| `where` | the requirement-clause position of an eligible declaration |
| `any` | immediately before a complete structural callable type |
| `some` | the start of an opaque callable result |
| `from` | immediately after a callable result type |
| `default` | before an interface default `method` |
| `coerce` | the start of an `instance` member |
| `drop` | the start of a top-level drop declaration or a `drop Name` statement |
| `static` | the start of a top-level static declaration or an explicit static provenance origin |
| `self` | the fixed receiver position of methods, operators, coercions, and drop declarations |
| `Self` | a type atom or construction-owner head in a type-owned context |
| `error` | a built-in type atom or construction-owner head; otherwise an ordinary value name |
| `_` | a discard binding, catch discard, payload slot, or match fallback |
| `target` | immediately after `#` in an item prefix |
| package directive names | immediately after `#` in a source directive prefix |

Outside those positions, an otherwise valid identifier spelling remains ordinary unless a topical
semantic rule forbids that declaration name. `alloc`, `import`, and `trait` have no contextual
production. Forms such as `alloc func`, `import ...`, and `trait ...` are invalid declarations.

## Keyed Packs and Mapping Literals

The language implements the following productions.

```text
KeyedPackParameter = "..." Name ":" Type ":" Type
KeyedCallableParameter = "..." Type ":" Type
KeyedCallArgument = Expression ":" Expression

KeyedForStatement = "for" Name ":" Name "in" HeaderExpression Block

MappingLiteralDeclaration = "literal" "[" ":" "]"
                            "(" KeyedPackParameter ")"
                            CallableTail CallableBody

TypedMappingLiteral = OwnerReference gap MappingBody AllocationOverride?
MappingBody = "[" (":" | NonEmptyList(MappingElement)) "]"
MappingElement = Expression ":" Expression
```

One callable admits at most one final ordinary or keyed pack. `KeyedPackParameter` extends a named
callable declaration's final parameter, while `KeyedCallableParameter` extends a structural
callable type's final parameter. `KeyedCallArgument` extends `CallArgument` and is accepted only in
the final keyed-pack contribution of a call. `KeyedForStatement` extends `ForStatement` and is
accepted only when its source resolves to the current callable's keyed pack. These semantic
restrictions do not make parsing depend on name or type resolution.

`MappingLiteralDeclaration` extends `ConstructMember`, and `TypedMappingLiteral` extends
`TypedLiteral`. `Type [:]` is the only empty mapping body. `Type []` remains an empty sequence, and
bare `[elements]` remains a fixed-size array. A mapping body has no spread production.
The complete ownership and ABI rules live in
[Argument Packs, Literal Definitions, and Sequence Spread](literals-and-packs.md).

## Future Direction: v2.0.0 Access Syntax

These productions close the S12 value/input/callable/alias source boundary. They are separate
from the released grammar. The complete declaration/module, literal/operator, generic-requirement,
platform and standard-library migrations are not implied by these productions. The implementation
design records their remaining entry gates; no old grammar is used as a fallback.

```text
V2InputOperation = "edit" | "take"
V2BindingOperation = "let" | "var" | "read" | "edit"
V2InvocationKind = "func" | "mut" | "once"

V2Type = V2TypeHead V2OutcomeSuffix?
V2OutcomeSuffix = "?" "?"* "!"? | "!"
V2TypeHead = "*" V2TypeHead | "future" V2Type | V2TypeAtom
V2TypeAtom = BuiltinScalarType | "str" | "error" | "void" | "never"
           | V2NamedType | "[" V2Type "]" | "[" V2Type ";" V2Expression "]"
           | "(" V2Type ")" | "(" V2Type "," V2Type ("," V2Type)* ","? ")"
           | V2CallableType | "any" V2CallableType
V2NamedType = (Name V2GenericArguments? | "Self") ("." Name V2GenericArguments?)*
V2GenericArguments = "<" NonEmptyList(V2GenericArgument) ">"
V2GenericArgument = V2Type | StructuralConstantExpression

V2OriginClause = "from" V2Origin ("|" V2Origin)*
V2Origin = Name | "self" | "static"

V2Parameter = V2OrdinaryParameter | V2KeyedParameter
V2OrdinaryParameter = "..."? V2InputOperation? V2Pattern ":" V2Type V2OriginClause?
V2KeyedParameter = "..." V2InputOperation? Name ":" V2Type ":" V2Type V2OriginClause?
V2Parameters = "(" List(V2Parameter) ")"
V2CallableType = "const"? V2CallableKind "(" List(V2CallableParameter) ")"
                 ":" V2Type V2OriginClause?
V2CallableKind = "func" | "once" | "mut" V2OriginClause?
V2CallableParameter = V2KeyedParameter | V2OrdinaryCallableParameter
V2OrdinaryCallableParameter = "..."? V2InputOperation?
                              (V2Pattern ":" V2Type | V2Type) V2OriginClause?

V2FunctionHead = "const"? "async"? "func" Name
V2MethodHead = "const"? "async"? "method" V2Receiver "." Name
V2Receiver = V2InputOperation? "self"
           | "(" "edit" "self" V2OriginClause ")"
V2CallableTail = ":" (V2Type | "some" InterfaceApplication V2OutcomeSuffix?)
                  V2OriginClause? V2WhereClause?

V2Pattern = Name | "_" | "(" V2Pattern "," V2Pattern ("," V2Pattern)* ","? ")"
V2Binding = V2BindingOperation V2Pattern (":" V2Type)? "=" V2Expression
V2Return = "return" V2Expression?
V2Throw = "throw" V2Expression
V2Drop = "drop" Name
V2IndexProjection = "operator" "(" "edit"? "self" "[" V2Parameter "]" ")"
                    ":" V2Type V2OriginClause? V2WhereClause? V2Block?

V2Closure = "(" V2ClosureHead ")" (":" V2Type V2OriginClause?)? V2Block
V2ClosureHead = (V2ValueCaptures ";" V2ClosureParameters? | V2ClosureParameters)?
V2ValueCaptures = NonEmptyList(Name)
V2ClosureParameters = NonEmptyList(V2ClosureParameter)
V2ClosureParameter = V2InputOperation? V2Pattern (":" V2Type)?
```

`BuiltinScalarType`, `Name`, `List`, `NonEmptyList`, `InterfaceApplication` and the constant-expression
subset denote their lexical/structural responsibilities, with recursively used types replaced by
V2 types. `V2WhereClause` retains the selected generic requirement families; its complete migration
must remove retired authored effect guarantees before any parser claims the full declaration
grammar. These are intentional named dependencies, not a claim that the fragment is a closed
standalone parser grammar. Named function/method heads take the full generic list, parameter list
and callable tail before an optional body under the owning module rules.

No ordinary type or expression has an `&`/`^` access prefix. No result marker occurs before a
name. Parameters use `edit value: T` or `take value: T`; anonymous callable inputs use `edit T`
or `take T`. Recognize a contextual input-operation word only when a following pattern/type
completes that production; an identifier such as `edit: T` is an ordinary unmarked name.
Variadic `...` precedes the input operation and remains limited to an eligible final parameter.
Pattern recognition does not prove arity, Copy, extraction rights or drop validity.
A keyed pack always names its binding, including in a callable type:
`func(... entries: K: V): R` or `func(... take entries: K: V): R`.
`func(... K: V): R` has only the ordinary named-pack interpretation (name K, element type V),
never a key/value interpretation selected by type lookup. Keyed pack names bind semantic input
positions just as other named callable inputs do; the names are not part of type identity.
Recognition does not bypass the separate pack/body/ABI migration gate.

An origin on an edit input is its post-state bound. Origins on read/take inputs constrain incoming
carried provenance under the owning type/schema contract; they never turn a read into an edit or
an owner. Names resolve against the complete header, including later inputs, not source order.
The parenthesized edit receiver permits its own post-state bound without overloading the method
result's clause. Unqualified `self`, `edit self` and `take self` are the only simple receiver forms;
there is no expanded typed receiver or optional symbolic form.

`mut from input (...)` describes the surviving environment's post-state. The clause after the
result type instead describes the result. A `func` or `once` head cannot have the mutating-state
clause. A callable result consumes its complete V2 type, so `func(i32): File!` and
`(func(i32): File)!` have different constructor trees. Grouping preserves those distinctions.

An explicit value-capture segment must be nonempty and ends with `;`. `(file; path: Path) { ... }`
and `(file;) { ... }` capture values; `(path: Path) { ... }` has no explicit capture segment.
The old `(^file; ...)` has no production. An expected callable contract can supply omitted closure
input operations; explicit operations must agree. Without an expected callable contract,
unmarked closure inputs default to read before checking the body. Body use cannot silently select
edit/take. Capture and data-type inference remain separate. Modes are never chosen from a call's
spelling or its later use. A closure result annotation does not duplicate a result access permission.

An index projection is the special existing-element protocol defined by the
[future indexing rules](equality-and-indexing.md#future-direction-v2-0-0-index-projections).
Its `return place`/final place result is not an ordinary function's value result. Read/edit
projection identities are selected from the source operation before authority checking; no
permission-based fallback is admitted.

### Expressions, Places and Statement Boundaries

The binary arithmetic, conversion, comparison, short-circuit and recovery precedence levels keep
their structural order. At unary level retain `!` and `-`, with `await` and postfix outcome
extraction in their existing precedence positions. Remove unary `&`, `&+`, `^` and the `move`
production. A unary `&&` token cannot be split into access operators. Binary `&&`/`||` remain
short-circuit operations. An expression outcome layer still admits one suffix, `?` or `!`;
nested layers need grouping. Repeated type optionality is not repeated expression extraction.

Postfix member, tuple-position, index and call structure is retained. A selected generic call
also admits explicit type/constant arguments under the existing generic-call disambiguation
contract, including `decode<Config>(bytes)`. Interface disambiguation uses the adopted
`value.(Interface.member)(...)` form when ordinary lookup is ambiguous. No UFCS, call-site
receiver assertions or direct argument-mode assertions are added.

A `read`/`edit` initializer must denote an existing place: a name, a grouped place, or its checked
field/element/range projection. Parsing retains a non-place initializer for a focused diagnostic;
lookup/type analysis does not reparse it. These declarations activate access immediately. A plain
value-binding initializer instead constructs a value even if it is a bare place. Calls select
their input operations before planning direct-place preparation; a computed conditional operand
has already constructed its ordinary branch value and cannot be reinterpreted as an alias.

Blocks retain a prefix of local `use` declarations, newline-separated statements, a possible
final expression and ordinary unit completion. A non-final expression must be unit or terminating.
Assignment remains a statement with RHS-first semantics; it has no expression precedence.
`return` without a same-line operand is bare unit return. `throw` requires an operand. `_` discards
one leaf; a complete discard statement uses only `let _`, without a type annotation. Alias
patterns require at least one named leaf. Neither aliases nor fixed owners can be retargeted by
assignment. Control/iteration patterns use the selected common tuple-pattern contract; a binding
pattern's operation applies consistently to its leaves.

A valid source program cannot depend on the retired `pub`/`construct`/`instance` or authored
`noalloc`/`notrap`/`blocking` roles. Complete future item grammar must enforce contract-only roots,
one-item path visibility and unified `impl` before module discovery is integrated. Reusing the
released `SourceFile` or `Item` productions without that migration is not permitted.

### First Access Validation Fragment

The pre-S12 `V2CoreUnit` implementation used a different grammar and value model. Its former
contract is preserved in the non-normative
[checkpoint snapshot](../../development/history/reviews/v2.0.0-access-checkpoint-contracts.md).
It is not a subset of the adopted S12 language and must not be exposed as a compatibility mode.
The [implementation design](../../development/history/reviews/v2.0.0-value-binding-implementation.md)
sets the coherent replacement boundary: declarations, typing, initialization, accesses, resources
and source fixtures must share one accepted-unit authority. The
[bounded implementation record](../../development/history/reviews/v2.0.0-value-binding-front-end.md)
distinguishes that replacement from the earlier checkpoint and from full-language support.

### Bounded Value Binding Fragment

The first S12/S16 replacement uses the internal `V2CoreUnit` goal below for definitions in one
supplied private module. This is its new source contract, not a compatibility variant or a public
package format. Its implementation and qualification are tracked in the
[development record](../../development/history/reviews/v2.0.0-value-binding-front-end.md).
The old corpus cannot qualify this grammar by relabeling its status. No bodies are thereby
permitted in contract-only `index.nct`.

This fragment imports future Name/literal/trivia/token/newline rules, `List`/`LineSequence`, and
`V2InputOperation`. It imports no released type, statement or expression root:

```text
V2CoreUnit = LineSequence(V2CoreFunction | V2CoreRecord)
V2CoreRecord = "copy"? "struct" Name "{" LineSequence(Name ":" V2CoreType) "}"
V2CoreFunction = "func" Name "(" List(V2CoreParameter) ")" ":" V2CoreType V2CoreBlock
V2CoreParameter = V2InputOperation? Name ":" V2CoreType
V2CoreType = Name | "void"
V2CoreBlock = "{" LineSequence(V2CoreEntry) "}"
V2CoreEntry = V2CoreStatement | V2CoreExpression
V2CoreStatement = ("let" | "var" | "read" | "edit") (Name | "_")
                   (":" V2CoreType)? "=" V2CoreExpression
                 | V2CorePostfix V2CoreAssignment V2CoreExpression
                 | "return" V2CoreExpression? | "drop" Name
                 | "break" | "continue"
                 | "while" V2CoreExpression V2CoreBlock | "loop" V2CoreBlock
V2CoreAssignment = "=" | "+=" | "-=" | "*=" | "/=" | "%="
V2CoreExpression = V2CoreOr
V2CoreOr = V2CoreAnd ("||" V2CoreAnd)*
V2CoreAnd = V2CoreEquality ("&&" V2CoreEquality)*
V2CoreEquality = V2CoreOrdering (("==" | "!=") V2CoreOrdering)?
V2CoreOrdering = V2CoreAdd (("<" | "<=" | ">" | ">=") V2CoreAdd)?
V2CoreAdd = V2CoreMultiply (("+" | "-") V2CoreMultiply)*
V2CoreMultiply = V2CorePrefix (("*" | "/" | "%") V2CorePrefix)*
V2CorePrefix = ("!" | "-") V2CorePrefix | V2CorePostfix
V2CorePostfix = V2CorePrimary (("(" List(V2CoreExpression) ")") | ("." Name))*
V2CorePrimary = Name | integer_literal | "true" | "false" | "(" ")"
               | "(" V2CoreExpression ")" | V2CoreIf
V2CoreIf = "if" V2CoreExpression V2CoreBlock
           ("else" (V2CoreBlock | V2CoreIf))?
```

Call openers stay on their callee's logical line. Parentheses preserve a place's category. Binary levels
associate left; comparisons allow one ungrouped operator; short-circuit right operands are
conditional edges. A return without a same-line operand is bare unit return. Blocks supply their
final expression or normal unit completion. A non-final expression must be unit or terminating.
Recognized non-place alias initializers/assignment targets are semantic errors, not alternate parses.
A complete discard is exactly `let _ = expression`, without an annotation; other `_` bindings
in this bounded grammar fail validation.

Resolve all named headers before bodies. Every parameter's mode comes from its head: read when
unmarked, otherwise edit/take. The result is always a delivered value, never I/U access. No
body-selected mode, result qualifier, `from`, generic, receiver, closure,
pack, outcome, async, const or module production is inherited into this fragment. Such constructs
are outside this private goal, not prohibited in the complete language. Direct calls resolve one
named function (including transparent grouping) and its argument count/contract before permission
checks. Functions are not storage places. A place here is a local/input name followed by zero or more declared record-field selections,
with transparent grouping. Resolve each field once through its containing record; two distinct
fields are disjoint logical places, while a field overlaps its containing place. Member selection
from computed values, indexing and record literals remain outside this bounded fragment. Record
values enter through declared inputs/results and ordinary value bindings; no constructor is
implied. Record data names resolve before bodies, including forward references. Follow
[nominal payload rules](structs-and-enums.md#future-direction-v2-0-0-nominal-value-payloads) for
finite containment, explicit Copy opt-in and closed field types. All fields are private-unit
members in this fragment; visibility and module contracts are not inherited.

Use the owning value-binding algorithm: Copy snapshots readable admitted data; otherwise move an
eligible owner. Read/edit aliases never supply non-Copy extraction. Fixed owners may transfer/drop
but cannot be assigned; mutable owners and take inputs can update/reinitialize their slots. Edit
aliases/inputs update their original places and must remain valid on outward recoverable exits.
Read inputs remain read even for Copy data, and a read Copy input can supply a separate take
snapshot. Two taking uses of a known-Copy scalar can succeed; two transfers of one non-Copy owner
cannot commit together. There are no source-level access/transfer prefix operations.

`bool`, `i32` and `void` are Copy and have empty carried-access/external-storage requirements and
trivial cleanup. The opaque move-only `error` owns its private immutable graph under the
[error contract](../../std/error/README.md#future-direction-v2-0-0-value-access); it has empty
outside D/S and graph cleanup. These are exact admitted data contracts, not implications of Copy
or value ownership. No constructor or failure outcome is introduced for error by this fragment.
A read error parameter therefore cannot be returned as an error value; a take error can transfer
it. Closed record payloads compose these exact leaf contracts and retain their schema cleanup.
All admitted value results are independent of callee-local storage under those contracts. Ordinary read/
edit aliases can still have R into local/input storage and need ordinary access validation.

Primitive arithmetic/ordering use i32, logic uses bool, and equality compares matching bool/i32/
void values under the [primitive equality rule](equality-and-indexing.md#future-direction-v2-0-0-core-primitive-equality).
Uncontextualized integer literals default to i32; decode magnitude before range checks and admit
`-2147483648` through grouping of its literal operand. Preserve checked overflow/division/remainder
semantics and each prepared scalar payload. Unit `()` is the one void value. A non-producing local
initializer cannot infer a data type; an annotation can provide it. Termination does not invent a
stored value or a continuing flow edge.

At ordinary conditional/block value boundaries, normally producing branches apply Copy-or-move
to a common inferred/expected data type. A terminating branch contributes no answer. An if without
else completes with unit and its normally producing then branch must be unit. A computed conditional
operand has already constructed its answer; a direct named operand instead follows the selected
call's read/edit/take preparation. Do not reinterpret computed moves as delayed direct reservations.
A condition that terminates contributes no continuing branch; structurally unreachable code still
has its own name/type obligations without fictitious runtime resource reads.

Assignment is RHS-first. Reinitialize only writable owning slots; clean initialized old values
conditionally and never read missing ones. Compound assignment reads its target only after the
right-hand side. Propagate definite initialization, access demand and owner state through every
reachable branch/backedge/exit; include zero-iteration while paths and actual break/continue
successors. Non-breaking loops and literal-true while without a reachable break have no normal
completion. Never derive a continuing edge from a dead break or choose Copy from later liveness.

Stage value results before normal cleanup. Track owner responsibility in one ledger and alias/
reservation relationships in the shared graph; access validation cannot invent a second survival
set. Validate all definitions, including unused/recursive ones, before publishing an accepted unit.
Exact source rebinding preserves the same immutable declaration identities and every body.
No target product, native ABI, CLI mode or released-program adapter follows from this private
front-end acceptance. The complete model's capability/state-footprint obligations remain required
before admitting dependent aggregates or callable values.
