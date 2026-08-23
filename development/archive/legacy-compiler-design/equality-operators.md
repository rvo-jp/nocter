# Equality Operator Architecture

Public syntax and semantics are specified in
[Values and Types](../../../spec/02-values-types.md) and
[Generics, Interfaces, and Methods](../../../spec/08-generics-interfaces-embedding-methods.md). This
document records the compiler boundaries for v0.12.0 Phase 1.

## Authored and Resolved Models

`ComparisonOperatorDecl` with the equality kind is an instance member with an ordinary visibility
boundary, fixed receiver shape, named right binding, `bool` result, and body.
`OperatorRequirementPredicate` is a distinct
where-clause node. Neither representation is encoded as an interface, a method spelling visible to
users, or free-form operator-overload metadata.

Resolution places the callable form of an equality declaration in the owner's ordinary static
method set under a compiler-private identity. That identity lets method visibility, declaration
patterns, qualification, cross-source bodies, specialization, call targets, occurrences, and
static lowering reuse existing services. Presentation always maps the identity back to authored
operator syntax and never exposes the private name.

## Selection and Plans

Type checking first tries an equality declaration on the exact left owner. If none applies, it asks
the common receiver-coercion candidate service for one-step readonly targets. The selected right
parameter is then checked exactly or through one readonly coercion. Owned operands receive an
implicit readonly borrow adjustment; already borrowed operands retain their capability. More than
one viable left-coercion target is a focused ambiguity, not a missing-operation error.

One shared `TypecheckComparisonPlan` records:

- source spans and concrete operand types;
- selected callable declaration identity and concrete `Self` type;
- left and right conversion plans;
- the right implicit-borrow adjustment.

The comparison kind keeps equality evidence independent from strict ordering while using the same
selector and downstream fact shape. Specialization substitutes unresolved generic operand types and rebuilds this plan once concrete
types are known. Ownership, provenance, buildability, IR, and editor consumers use the recorded
plan. They must not look up `==`, inspect a standard type name, or select a coercion again.

## Lowering and Ownership

Primitive booleans, integers, `str` data, and payloadless enum tags retain leaf equality lowering.
Source-defined equality lowers through the ordinary static-call boundary. Receiver preparation and
the implicit right borrow evaluate each operand once, apply their recorded conversion at most once,
and preserve the owners after the call. `!=` wraps the same result in logical negation.

Generic operator requirements are compile-time evidence. They add no witness, metadata, or ABI
field. A specialized call must construct a concrete equality plan before IR lowering.

## Standard-Library Boundary

`std/str` owns byte equality. `String` reaches it through its existing readonly `str` coercion.
`std/slice` owns element equality and readonly search under `where (&T == &T): bool`; `Vec<T>` reaches that
surface through its slice coercion. Iterator default methods use the same requirement and borrow
each yielded owner only for the comparison so ordinary cleanup remains authoritative.

No compiler table names these types or APIs. The built-in-instance authority model is the only
exception that permits the selected standard package to attach the source declaration to `str` or
`[T]`.

## Analysis Boundary

Hover and completion format the authored declaration with a concrete owner. Definition,
references, and rename use the declaration identity at the `==` token. The right binding retains
its independent parameter identity. Semantic tokens classify `operator` as a keyword, `==` as a
method declaration, and both operands as readonly parameters.

Analysis consumes resolver identities and typecheck plans. It must not scan text for operator
shapes or expose the compiler-private callable identity.
