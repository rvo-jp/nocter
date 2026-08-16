# Syntactic Grammar

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

This chapter is the normative recognition grammar for the source forms whose productions it
defines. The
[Lexical Grammar](13-lexical-grammar.md) owns token formation, comments, newline tokens, and the
common comma-delimited-list rule. Topical chapters own name resolution, visibility reachability,
typing, ownership, evaluation, and container-specific validity after a source form has been
recognized.

`Block` and `Expression` are referenced here but remain defined by the control-flow and expression
chapters until their productions are consolidated into this chapter. A referenced production is
not an invitation for a compiler to infer syntax from examples: only an explicit rule in the
linked chapter may recognize it.

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
Delimited(X) = X ("," X)* ","?
LineSequence(X) = newline* (X (newline+ X)*)? newline*
```

`Name` means an `identifier` token unless a narrower production says otherwise. `LineSequence`
owns separation for declarations that do not use commas: adjacent elements require at least one
newline, while leading, trailing, and blank lines are accepted. A leaf declaration never consumes
its enclosing separator. Newlines accepted as continuations inside parameter lists, generic lists,
directives, types, requirements, and expressions are not separators in `LineSequence`.

Comments do not change syntactic recognition. Documentation comments are retained as source
metadata and attach according to their lexical kind; removing them leaves the same token grammar.

## Source Files

Nocter recognizes package files separately from module source files:

```text
PackageFile = LineSequence(PackageDirective) eof

ModuleSource = newline*
               (UseSequence (newline+ ItemSequence)? | ItemSequence)?
               newline* eof

UseSequence = UseDeclaration (newline+ UseDeclaration)*
ItemSequence = Item (newline+ Item)*
```

Only `nocter.nct` is a `PackageFile`. It accepts package documentation and package directives, but
not imports or declarations. Every other selected `.nct` file is a `ModuleSource`. A module source
places every top-level `use` before its first item. Whether a module source is an `index.nct` module
root or a same-module implementation source does not change its grammar; the module rules reject
visibility and member forms that an implementation source is not allowed to contribute.

The grammar deliberately does not accept an arbitrary mixture of imports and items. A later
top-level `use` is a syntax error even when name resolution would otherwise succeed.

## Package Directives

```text
PackageDirective = "#" PackageDirectiveName ":" DirectiveValue

PackageDirectiveName = "name"
                     | "version"
                     | "dependencies"
                     | "lock"
                     | "executable"
                     | "test"

DirectiveValue = string_literal
               | integer_literal
               | DirectiveRecord

DirectiveRecord = "{" Delimited(DirectiveField)? "}"
DirectiveField  = Name ":" DirectiveValue
```

The closed directive names, allowed repetitions, and record schemas are defined by
[Package File](15-command-line-interface.md#package-file). The recursive record grammar only
recognizes their common data notation; it does not permit a schema to accept arbitrary fields or
value kinds.

For example:

```nct
#name: "example"
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

## Use Declarations

```text
UseDeclaration = Visibility? "use" UseTree

UseTree = ModulePath
        | ModulePath "." ImportSelection

ImportSelection = Name ("as" Name)?
                | "{" Delimited(SelectedName) "}"

SelectedName = Name ("as" Name)?

ModulePath = PackageModulePath | RelativeModulePath

PackageModulePath  = ModuleSegment ("/" ModuleSegment)*
RelativeModulePath = ("." "/" | ("." "." "/")+) ModuleSegment ("/" ModuleSegment)*
```

`ModuleSegment` is the snake-case module segment defined by the lexical grammar. Resolution
decides whether a private bare relative path is a same-module source import or whether a path is a
directory-module import. The parser does not choose between those meanings.

A block-scope import uses the private `"use" UseTree LineEnd` form. Visibility is recognized only
on a top-level use declaration; the module rules further reject visibility on a same-module source
import and reject namespace alias re-exports.

## Items

```text
Item = TargetDirective? TargetableItem
     | ConstructDeclaration
     | InstanceDeclaration
     | ConformDeclaration
     | DropDeclaration
     | TestDeclaration

TargetableItem = FunctionDeclaration
               | PrimitiveDeclaration
               | TypeAliasDeclaration
               | StructDeclaration
               | EnumDeclaration
               | InterfaceDeclaration

TargetDirective = "#" "target" ":" string_literal newline+
```

The grammar makes the `#target` attachment structural: it prefixes exactly one targetable item.
It cannot prefix a `use`, `construct`, `instance`, `conform`, `drop`, or `test`, and it cannot occur
without a following targetable item. Target and standard-library authority checks happen after
parsing.

## Shared Declaration Parts

The following productions are reused by declarations and member containers:

```text
GenericParameters = "<" Delimited(Name) ">"

Parameters = "(" Delimited(Parameter)? ")"
Parameter  = Name ":" Type

CallableTail = ":" CallableResult ProvenanceClause? WhereClause?
ProvenanceClause = "from" ProvenanceOrigin ("|" ProvenanceOrigin)*
ProvenanceOrigin = Name

CallableBody = Block?
```

`CallableResult`, `Type`, and `WhereClause` are defined below. A bodyless declaration ends when its
enclosing `LineSequence` reaches a separator or closing brace; later semantic validation permits
it only for a source form that owns an external contract/body split or is intrinsically bodyless.

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
                    "{" LineSequence(StructField) "}"

StructField = Visibility? Name ":" Type

EnumDeclaration = Visibility? "enum" Name GenericParameters? WhereClause?
                  "{" LineSequence(EnumVariant) "}"

EnumVariant = Name EnumPayload?
EnumPayload = "(" Delimited(Parameter)? ")"
```

Struct fields and enum variants are newline-separated declarations. They are not comma-delimited
items. Enum payload parameters are comma-delimited. The grammar recognizes an empty enum body so
the enum validity rule can emit its dedicated “at least one variant” diagnostic; a valid enum has
one through 256 variants.

`copy` is contextual only immediately before `struct`. It is not a general type modifier and does
not occur on an enum or alias.

## Interfaces

```text
InterfaceDeclaration = Visibility? "interface" Name GenericParameters? WhereClause?
                       "{" LineSequence(InterfaceMember) "}"

InterfaceMember = AssociatedTypeDeclaration | InterfaceMethod

AssociatedTypeDeclaration = "pub" "type" Name InterfaceBounds?
InterfaceBounds = ":" Capability ("+" Capability)*

InterfaceMethod = "pub" MethodSignature CallableBody
```

An interface member always writes bare `pub`; it cannot narrow its visibility independently from
the interface. A bodyless method is a requirement. A method with a block is a default method.
Fields, operators, coercions, construction entries, drop declarations, and tests have no interface
member production.

## Construction Declarations

```text
ConstructDeclaration = "construct" DeclarationTypePattern
                       "{" LineSequence(ConstructMember) "}"

ConstructMember = Visibility "default"? ConstructionFunction
                | Visibility "default"? LiteralDeclaration

ConstructionFunction = "func" Name GenericParameters? Parameters
                       CallableTail CallableBody

LiteralDeclaration = "literal" LiteralShape LiteralParameters CallableTail CallableBody

LiteralShape = "[" "]" | empty_string_literal

LiteralParameters = "(" "..." Name ":" Type ")"
                  | "(" Parameter ")"
```

`empty_string_literal` is the `string_literal` token whose source and decoded contents are both
empty (`""`). In this position it denotes a literal shape, not a
decoded runtime string value. Semantic validation pairs the sequence shape with its one element
pack and the string shape with its one ordinary parameter. Every construction member requires an
explicit `Visibility`; `default` is contextual only between that visibility and `func` or
`literal`.

An empty construction body is syntactically valid and explicitly declares no direct construction
entry.

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
                   ":" "bool" WhereClause? Block

OrderingOperator = Visibility? "operator" "(" "&" "self" "<" Name ":" "&" "Self" ")"
                   ":" "bool" WhereClause? Block

IndexOperator = Visibility? "operator" "(" IndexReceiver "[" Parameter "]" ")"
                ":" BorrowType ProvenanceClause? WhereClause? Block
IndexReceiver = "&" "self" | "&+" "self"

ExpansionOperator = Visibility? "operator" "(" "..." ExpansionReceiver ")"
                    ":" Type ProvenanceClause? WhereClause? Block
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
ConformMethod = MethodSignature Block
```

Conformance members never write visibility. They contain only associated type bindings and
body-bearing method implementations. Construction entries, fields, operators, coercions, drop
declarations, tests, and extra functions have no conformance-member production.

## Declaration Type Patterns

```text
DeclarationTypePattern = Name PatternArguments?
                       | BuiltinTypePattern
                       | "[" Name "]"

PatternArguments = "<" Delimited(Name) ">"
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

construct ExternalType {}
```

The first is invalid unless it is an eligible public contract with one matching same-module body;
the second violates the enum variant-count rule; the third violates construction ownership unless
the resolved target belongs to the declaring module or to the authorized standard-library
surface. Keeping those checks outside parsing gives every accepted token sequence one stable
syntax-tree shape.

## Types

```text
Type = PrefixType OutcomeSuffix?

OutcomeSuffix = "?" "!"? | "!"

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
         | "Self"
         | NamedType
         | SliceType
         | FixedArrayType
         | GroupedType

BuiltinScalarType = "bool"
                  | "i8" | "i16" | "i32" | "i64"
                  | "u8" | "u16" | "u32" | "u64"
                  | "usize" | "isize"

NamedType = Name TypeArguments? ProjectionSuffix*
TypeArguments = "<" Delimited(Type) ">"
ProjectionSuffix = "." Name

SliceType = "[" Type "]"
FixedArrayType = "[" Type ";" integer_literal "]"
GroupedType = "(" Type ")"

CallableType = CallableCapability "func" "(" Delimited(CallableParameter)? ")"
               ":" Type ProvenanceClause?
CallableCapability = ("&" | "&+")?
CallableParameter = Type | Name ":" Type

BorrowType = "&" NonCallablePrefix | "&+" NonCallablePrefix

CallableResult = Type | OpaqueResult
OpaqueResult = "some" Name OpaqueArguments?
OpaqueArguments = "<" Delimited(OpaqueArgument) ">"
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
structural callable type.

Because `&func` and `&+func` begin callable types, they are not parsed as an ordinary borrow prefix
followed by a separate `func` type. Borrowing a callable type as data requires explicit grouping,
just like borrowing another already composed type.

An associated projection is a suffix of a named type atom. Name resolution and conformance decide
whether `Base.Name` denotes one valid associated type; the parser does not reinterpret it as a
module path. A projection may then participate in ordinary outer prefix and outcome forms.

The fixed-array length is an integer-literal token. General constant expressions and const generic
parameters have no type production.

The lexer emits `>>` as one punctuation token. While parsing type arguments, that token supplies
two consecutive `>` closers only when two currently open type-argument lists require them. Thus
`Outer<Inner<T>>` is canonical and does not require whitespace between the closers. In expression
grammar, the same token remains the right-shift operator; no name or type information participates
in this decision.

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
ExpansionRequirementSource = Type | "&" Type | "&+" Type
```

The same comma-separated predicate grammar applies after functions, methods, aliases, nominal type
declarations, construction members, instances, conformances, and operators wherever their
declaration production includes `WhereClause`. A newline is not a predicate separator.

The parser records the predicate form without proving it. Later validation distinguishes a
general associated-type equality from a declaration-pattern binder refinement, restricts
structural operator operands, checks callable capability multiplicity, and rejects duplicate or
unsatisfied requirements.
