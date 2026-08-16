# Destruction Declaration Architecture

Public syntax and semantics are specified in
[Ownership, Borrowing, and Drop](../../spec/05-ownership-borrowing-drop.md). This document owns the
compiler boundary introduced in v0.11.0 Phase 6.

## Declaration Boundary

`DestructDecl` is a top-level AST item with an authored keyword span, declaration type pattern,
derived generic binders, fixed `&+self` parameter, and body. It has no visibility, `where` clause,
return contract, interface identity, or callable source name. `InstanceDecl` stores methods only.

The parser reserves `destruct` and accepts exactly `destruct TypePattern(&+self) { ... }`. It derives
binder identities through the same declaration-pattern parser used by `instance` and `conform`.
Semantic validation requires a nominal struct or enum target, one distinct binder per nominal
generic slot, and no copy target. Resolver collection enforces one destructor per nominal family.

## Semantic Identity

The resolver retains one `DestructSignature` on the nominal `TypeSymbol` because cleanup lookup is a
property of the type. That signature is sourced only from `DestructDecl`; it does not make the
destructor an inherent method. Its declaration identity is the `destruct` keyword span.

Automatic cleanup, explicit `drop value`, field cleanup, failure cleanup, buildability,
specialization, and IR drop glue consume that same identity. Generic specialization unifies the
destructor target pattern with the concrete self type through the shared declaration-pattern
substitution service.

## Body and Editor Traversal

Every exhaustive callable-body walker handles `Item::Destruct` directly. Body resolution and type
checking define exactly one read-write borrowed `self` parameter in an environment containing the
target binders and specialized `Self`. Return checking fixes the result to `void`.

Formatting, AST JSON, documentation attachment, document symbols, hover, completion context,
visible locals, module paths, occurrences, semantic tokens, region analysis, and call-site lookup
use the authored keyword, target, receiver, and body spans. Normalized presentation is
`destruct Type(&+self)` and uses the visible type name rather than a filesystem-qualified canonical
name.

## Deliberate Exclusions

There is no conditional destruction, interface destructor, direct destructor call, visibility,
specialization ranking, bodyless destructor contract, or compatibility AST for the removed
`drop &+self` instance member. Any future conditional cleanup model must first define generic
ownership, layout, and ABI behavior rather than reuse method selection.
