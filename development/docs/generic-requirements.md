# Generic Requirement Architecture

Public syntax and semantics are specified in
[Generics, Interfaces, and Methods](../../spec/08-generics-interfaces-embedding-methods.md). This
document records the compiler boundary completed through v0.11.0 Phase 3.

## Representations

The AST represents a generic parameter as a name and source span only. Every constraint belongs to
one declaration-owned `WhereClause`. Its predicates retain separate spans for contextual keywords,
target names, capability types, and equality operands. `where copy T`, `where T: Interface`, and
`where L.Item = R.Item` therefore remain distinct authored nodes instead of overloading a type
expression or a parameter modifier.

Resolver signatures map predicates to lexical parameter identities. They store parameter
requirements in `GenericRequirements`, resolved type equalities, and structural equality-operation
requirements beside them. Each parameter
requirement has one semantic kind: nominal, callable, or intrinsic copy. Consumers select the kind
they understand instead of reclassifying arbitrary `TypeExpr` values or inspecting formatted text.

`TypeEnvironment` carries the resolved requirements for every visible generic parameter. Interface
lookup consumes nominal requirements, callable invocation consumes callable requirements, and the
ownership classifier consumes `copy`. Concrete call and conditional-conformance matching use the
same classifier as generic-body ownership. Copy requirements produce no witness, ABI field, or
runtime metadata. Associated-type equality entailment and operator-capability lookup consume the
same type environment and produce no runtime witness.

## Declaration and Specialization Flow

1. Parsing records name-only parameter lists and declaration-wide `where` predicates without
   reserving `copy` or `where` globally.
2. Declaration validation resolves every predicate target in lexical generic scope and rejects
   duplicate or invalid requirement sets.
3. Resolver signatures derive every parameter requirement from the clause by parameter identity.
4. Generic-body checking treats `T` as copyable only when its environment contains the intrinsic
   requirement.
5. Call specialization validates the concrete substitution at the argument evidence span.
6. A nominal specialization validates its declaration's requirements before the type can be used
   as a field, parameter, result, conformance target, or nested type argument.
7. Associated-type bounds and equality predicates validate through the same resolved requirement
   and projection services.
8. Imported signatures qualify nominal bound and equality operand types while preserving
   intrinsic identities and source spans.
9. Equality requirements retain their structural operator identity and specialize through the
   same resolved equality plan as an ordinary expression.

AST JSON, normalized presentation, type occurrences, semantic tokens, signature help, and
diagnostics derive from these representations. Editor code must not scan source text to rediscover
requirements.

## Standard-Library Contract

Readonly generic copying must be stated at the public declaration. `Vec.from_slice`,
`Vec.try_from_slice`, and their top-level forwarding functions use `where copy T`. Moving iteration
and construction APIs remain unconstrained.

## Source Invariants

- `<T, U>` declares names and arity; `<copy T>` and `<T: Interface>` are rejected syntax.
- `where T: Capability` is reserved for interface and structural callable conformance.
- `where copy T` is the only intrinsic copy spelling; `where T: copy` is rejected.
- `where Left = Right` on an ordinary generic declaration relates types and requires at least one
  associated projection. `instance` and `conform` classify `where Binder = Type` separately as a
  directed declaration-pattern refinement; see
  [Declaration Type Pattern Architecture](declaration-type-patterns.md).
- `where &T == &T` is the only operator requirement. Both operands must name the same visible
  parameter; it is not a nominal interface bound or an associated-type equality.
- functions, methods, literals, nominal declarations, aliases, instances, and conformances all own the same clause
  representation; no declaration kind carries an inline fallback.
- associated type declarations may retain `pub type Item: Interface` because that bound constrains
  the type selected for the member, not a generic parameter.

Parser recovery, AST JSON, formatting, qualification, diagnostics, hover, completion, signature
help, and semantic tokens consume these authored nodes or their resolved identities. Editor code
must not scan source text or parse a presentation label to rediscover a requirement.
