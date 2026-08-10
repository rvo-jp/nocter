# Generic Requirement Architecture

Public syntax and semantics are specified in
[Generics, Interfaces, and Methods](../../spec/08-generics-interfaces-embedding-methods.md). This
document records the compiler boundary introduced for v0.11.0 Phase 0.

## Representations

The AST preserves authored inline parameters and shared callable/impl `where` clauses, including
separate spans for contextual keywords, target names, type bounds, and equality operands. Resolver
signatures merge parameter predicates into `GenericRequirements` and retain resolved type
equalities beside them. Each parameter requirement has one semantic kind: nominal, callable, or
intrinsic copy. Consumers select the kind they understand instead of reclassifying arbitrary
`TypeExpr` values or inspecting formatted text.

`TypeEnvironment` carries the merged requirements for every visible generic parameter. Interface
lookup consumes nominal requirements, callable invocation consumes callable requirements, and the
ownership classifier consumes `copy`. Concrete call and conditional-conformance matching use the
same classifier as generic-body ownership. Copy requirements produce no witness, ABI field, or
runtime metadata. Equality entailment consumes the same type environment and produces no runtime
witness.

## Declaration and Specialization Flow

1. Parsing records `<copy T>` and `where copy T` without reserving either contextual word globally.
2. Declaration validation resolves every target in lexical generic scope and rejects duplicate or
   invalid requirement sets.
3. Resolver signatures merge inline and callable requirements by parameter identity.
4. Generic-body checking treats `T` as copyable only when its environment contains the intrinsic
   requirement.
5. Call specialization validates the concrete substitution at the argument evidence span.
6. Associated-type bounds and equality predicates validate through the same resolved requirement
   and projection services.
7. Imported signatures qualify nominal bound and equality operand types while preserving
   intrinsic identities and source spans.

AST JSON, normalized presentation, type occurrences, semantic tokens, signature help, and
diagnostics derive from these representations. Editor code must not scan source text to rediscover
requirements.

## Standard-Library Contract

Readonly generic copying must be stated at the public declaration. `Vec.from_slice` and
`Vec.try_from_slice` use `where copy T` because `T` belongs to the surrounding construction owner;
their top-level forwarding functions declare `<copy T>`. Moving iteration and construction APIs
remain unconstrained.
