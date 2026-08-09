# Associated Type Identity and Projection Normalization

This document defines the compiler responsibility boundary for the associated type behavior in the
[language specification](../../spec/08-generics-interfaces-embedding-methods.md#associated-types).
The specification owns source semantics. This document owns the representation, normalization, and
consumer invariants that keep those semantics consistent.

## One Contract Identity

An associated type declaration belongs to an interface symbol. Its declaration span is the stable
source identity used by semantic analysis. A conformance binding and every `Self.Name`, `T.Name`,
or concrete projection point back to that declaration; they do not create independent type-member
identities.

The compiler preserves three authored forms:

- `AssociatedTypeDecl` stores an interface declaration and its focus span
- `AssociatedTypeBinding` stores a conformance selection and its value type
- `ProjectedType` stores the base type, member name, and independent member span

Resolver `TypeSymbol` data owns declarations. Resolver `InterfaceConformance` data owns authored
bindings. Downstream consumers must not rediscover either relationship by scanning an AST, parsing
canonical text, or recognizing `Iterator`, `Item`, `std`, or a module path.

## Normalization Boundary

Type checking represents an unresolved projection as a base type plus member name. The associated
type service performs both supported resolutions:

1. A generic base resolves the member against its merged inline and callable-requirement interface
   set and retains the projection until specialization.
2. A concrete base selects one applicable conformance, specializes that conformance's binding, and
   recursively normalizes the result.

Zero or multiple declaration candidates are semantic errors. Zero or multiple applicable concrete
conformances do not produce a guessed type. Type aliases are expanded through the ordinary type
conversion path before concrete conformance selection.

The normalization service is the only bridge from a projection to its selected value type.
Interface method compatibility, expression checking, ownership, copyability, sizing, provenance,
ABI classification, buildability, and IR lowering call that service instead of adding local
projection cases. A still-generic projection is conservative wherever a concrete layout or value
capability is required.

## Conformance Validation

Conformance checking validates the declaration set before method signatures:

- each required declaration has exactly one binding
- no binding names an absent declaration
- duplicate declarations and duplicate bindings are rejected
- inherent implementations cannot carry bindings

Method compatibility substitutes the target type, interface arguments, impl parameters, and the
validated associated bindings into the interface method signature. This permits an interface
result such as `Self.Item` to match a concrete implementation result such as `i32` without textual
equivalence.

## Analysis and Editor Boundary

Typecheck facts publish one type occurrence for every associated declaration binding or projected
use. Its target is the interface declaration span. The package occurrence index converts that span
to a semantic member identity, which drives hover, definition, references, rename, and semantic
tokens across files and imports.

Completion obtains candidates from the same resolved sources used by type checking:

- the current interface for `Self.`
- merged inline and callable requirements for a generic parameter
- the unique applicable conformance for a concrete base

Incomplete-source recovery may add a temporary identifier after a trailing dot, but candidates are
returned only when normal analysis resolves the base and interface identity.

## Deliberate Boundary

Required associated types do not solve equality constraints. Iterator adapters with two independent
sources still need an explicit item parameter until a later phase can express and prove a relation
such as `Right.Item = Left.Item`. Defaults, bounds, generic associated types, inherent associated
types, and associated constants also remain outside this subsystem rather than receiving partial
or name-based implementations.

## Verification

Focused tests cover parsing and recovery, stable formatting, AST JSON, missing and extra bindings,
duplicate and ambiguous projections, `Self`, inline and callable requirements, concrete and nested
normalization, aliases, imported declaration identity, semantic tokens, hover, completion,
definition, references, rename, and native generic specialization. The complete compiler gate must
also pass because projections cross ownership, buildability, and lowering boundaries.
