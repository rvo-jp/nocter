# Callable Values and Interface Default Methods

This document owns the compiler implementation design for v0.3.0 Phase 10. Public semantics belong
to the specification. The active completion gate belongs to the
[v0.3.0 Development Contract](v0.3.0.md).

## Separation of Responsibilities

Phase 10 uses two composition mechanisms:

| Mechanism | Responsibility |
|---|---|
| interface | declares required capabilities and reusable behavior derived from them |
| embedding | owns stored state and participates in layout, provenance, and cleanup |

A body-bearing interface method does not add layout or conformance. A closure environment is an
ordinary compiler-generated aggregate and is not an interface object.

## Required and Default Methods

`MethodDecl.body` is the semantic distinction: `None` is a conformance requirement and `Some` is a
default callable target. `MethodSignature` retains this declaration identity and body availability
through imports. Conformance validation compares only required signatures.

A default body is resolved and checked with `Self` bound to its declaring interface. It therefore
uses the same generic-bound lookup as an ordinary generic function and cannot access a concrete
type's private or structural surface.

Method lookup is deterministic:

1. select an applicable accessible inherent method
2. otherwise enumerate default methods from proven interface conformances or generic bounds
3. select exactly one declaration identity or emit an ambiguity diagnostic

Import order is never a tie breaker. The selected fact contains the interface declaration, method
declaration, concrete receiver type, generic substitutions, and callable target. Ownership,
provenance, allocation effects, buildability, IR, and LSP consume that shared fact.

## Closure Syntax and Capture Ownership

A closure is a value expression such as `(value) { value * 2 }`. Parameters may be inferred from
an expected callable contract or annotated explicitly. A tail expression produces the result;
`return` exits the closure body.

Captures precede a semicolon and are always explicit:

```nct
(&threshold; value) { value > threshold }
(&+count; value) { count += 1; value }
(move prefix; value) { prefix.view() == value }
```

Readonly, readwrite, and owned captures become fields in source order. A reference to an outer
binding that is not listed is rejected. Duplicate captures, capture/parameter collisions,
incompatible capture capability, and capture after move are source-backed diagnostics.

The compiler assigns each closure expression a stable declaration identity. Its anonymous concrete
type owns its capture fields and one generated call target. The value is statically specialized
through a trusted callable interface conformance; there is no heap box, erased environment, code
pointer ABI, vtable, or spelling-based `call` lookup.

Readonly call, mutable repeated call, and consuming call are separate receiver capabilities.
Iterator adapters use mutable repeated call. A closure satisfies that capability only when its body
does not consume captured state.

## Shared Closure Plan

Typecheck owns one immutable closure plan containing:

- closure declaration identity and anonymous type identity
- ordered capture places, modes, field types, and storage provenance
- specialized parameter and return types
- receiver capability and callable interface declaration identity
- body call target, allocation effect, and result provenance summary
- capture initialization, move, and cleanup obligations

Ownership consumes capture obligations from the plan. Provenance treats the closure as an aggregate
of capture fields. Allocation analysis follows the generated body target in the ordinary fixed
point. IR materializes the aggregate once and invokes the generated target directly.

## Standard Iterator Chain

The `Iterator<T>` interface owns cardinality-independent defaults and focused adapter modules own
their state:

- `std/iter/map` owns `MapIter`
- `std/iter/filter` owns `FilterIter`
- existing range, take, skip, chain, enumerate, and terminal implementations remain distributed
- `std/iter/collect` owns consuming `to_vec` support

Exact-size-only defaults live on `ExactSizeIterator<T>`. `map` preserves exact size when its source
is exact. `filter` never claims exact size. Every adapter owns its source and callback, allocates
nothing, and drops current state plus remaining source exactly once. `to_vec` uses the current
aborting allocation context through the existing vector builder.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| required/default method AST distinction | existing interface and method AST |
| default declaration identity through imports | `resolve/signatures` |
| conformance requirements | `resolve/conformance` and `typecheck/interfaces` |
| deterministic default selection | focused `typecheck/default_methods` |
| closure AST and recovery | focused modules under `ast` and `parser` |
| closure local/capture identity | `resolve/closures` |
| closure plans and contextual inference | `typecheck/closures` |
| capture loans, moves, and cleanup | existing `typecheck/ownership` consuming closure plans |
| anonymous layout and static invocation | focused `ir/lower/closures` support |
| editor-facing callable facts | compiler `analysis`; protocol conversion in `driver/lsp` |

## Verification

Focused tests cover required/default conformance, inherent override, generic and concrete lookup,
default ambiguity, method-generic scope, zero/one/two-parameter closures, every capture mode,
move-only cleanup, closure provenance and allocation effects, map/filter order, exact-size
preservation, terminal collection, region escape, incomplete editor input, and repository plus
packaged-home native execution.
