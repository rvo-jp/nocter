# Borrow Coercion Compiler Boundary

This document owns the compiler design for v0.8.0 borrowed-view coercions. Public syntax and
behavior belong in the specification; milestone scope and completion evidence belong in the
[v0.8.0 milestone](../milestones/v0.8.0.md).

## Responsibility Model

Borrow coercion is an implicit statically resolved call, not type equality and not an IR cast.

```text
coerce declaration
  -> resolver declaration identity and coherent key
  -> typechecked callable summary
  -> expected-type selection
  -> immutable CoercionPlan
  -> ownership, regions, analysis, and IR lowering
```

No consumer after typecheck searches declarations or recognizes source and target names.

## Declaration Identity

The coherent key consists of the source nominal declaration identity, receiver capability, and
canonical target type under the declaration's generic parameters. Visibility is attached to the
entry. Source and target spans remain available for diagnostics and semantic occurrences, but text
does not define identity.

Only the source type's defining module may add entries. This rule prevents import order and package
composition from changing the implicit conversion set.

## Contextual Plan

Expected-type checking may select one accessible entry after exact type compatibility fails. The
resulting immutable plan records:

- coercion declaration and source-module identity
- concrete source, receiver, and target types
- generic arguments substituted from the source nominal type
- receiver capability and any built-in capability weakening
- result provenance instantiated from `self`
- source and expectation spans
- concrete callable target used by lowering

The plan is keyed by the source expression span. Nested consumers reuse it instead of applying the
coercion again.

## Ownership and Provenance

The caller supplies an explicit borrow expression. Selection may reborrow `&+Source` as readonly,
but it never creates a borrow from an owned source. The returned view carries the original source
loan and uses the same non-lexical lifetime and region escape machinery as an explicit method call.

A readwrite result requires a readwrite receiver. Returning mutable access to invariant-bearing
storage remains the defining type's responsibility; `String` therefore exposes only `&str`, while
`Vec<T>` may expose `&+[T]`.

## Lowering

IR lowering evaluates the source once, materializes the receiver according to the plan, and invokes
the concrete coercion body through the ordinary static-call ABI. Coercion does not create a target
value by reinterpretation and does not bypass callable provenance or cleanup.

## Editor Boundary

AST source ranges feed semantic occurrences for `coerce`, the source type, `self`, `as`, the target
type, and `from self`. Hover and completion render normalized notation from the declaration model.
Definition, references, and rename use resolver identities. LSP transport only converts positions
and presentation blocks.

