# Callable Values and Interface Default Methods

This document records the compiler implementation design completed by v0.3.0 Phase 10. Public
semantics belong to the specification. The completion record belongs to the
[v0.3.0 Release Record](v0.3.0.md).

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

1. collect applicable accessible inherent methods and members or defaults from proven interface
   conformances
2. select exactly one declaration identity or emit an ambiguity diagnostic
3. for a generic receiver, search only the receiver's explicit interface-bound set

An inherent method does not override a conformance member or default. Import order is never a tie
breaker. The selected fact contains the contract declaration, dispatch declaration, concrete
receiver type, generic substitutions, and callable target. Ownership, provenance, allocation
effects, buildability, IR, and LSP consume that shared fact.

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
type owns its capture fields and one generated call target. A generic bound states the structural
contract as `&func(Input): Output`, `&+func(Input): Output`, or `func(Input): Output`. The value is
statically specialized through a dedicated callable-call fact; there is no heap box, erased
environment, code pointer ABI, vtable, standard-library protocol identity, or spelling-based
`call` lookup.

Readonly repeated call, mutable repeated call, and consuming call are separate capabilities. Direct
source invocation is always `callback(...)`. A readwrite call requires a writable callable place;
a consuming call moves that place. Iterator adapters use mutable repeated call. A closure satisfies
that capability only when its body does not consume captured state.

## Shared Closure Facts

Typecheck records a compact immutable closure plan containing the source span, normalized anonymous
closure type, and generated call target. A separate callable-call fact owns the checked signature,
capability, concrete anonymous type, and static specialization. Keeping it separate from method
specialization preserves the closure declaration's source identity and prevents built-in calls from
becoming synthetic method calls. Ordinary compiler facts remain the owners of their domains:
ownership tracks capture moves and loans, provenance tracks aggregate fields and call results,
allocation analysis follows the generated target, and IR materializes the closure aggregate once
before invoking that target directly.

This split prevents a second ownership or effect model from accumulating inside closure support.
Recursive concrete drop-dependency discovery follows the closure and iterator field graph, so
nested moved captures and adapter sources receive the same generated cleanup as named aggregates.

## Standard Iterator Chain

The `Iterator<T>` interface owns cardinality-independent defaults. `std/iter/core` owns `MapIter`
and `FilterIter` beside those defaults because Nocter requires inherent implementations to share
their declaration module and moving them elsewhere would create a module cycle. Existing range,
take, skip, chain, enumerate, and terminal free-function surfaces remain in focused Phase 9 modules.

Exact-size-only defaults live on `ExactSizeIterator<T>`. `map` preserves exact size when its source
is exact. `filter` never claims exact size. Every adapter owns its source and callback, allocates
nothing, and drops current state plus remaining source exactly once. `to_vec` uses the current
aborting allocation context through the existing vector builder.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| required/default method AST distinction | existing interface and method AST |
| default declaration identity through imports | `resolve/signatures` |
| conformance member identity | `resolve/conformance` |
| conformance requirements | `typecheck/interface_impl_members` |
| deterministic interface member selection | `typecheck/interface_methods` |
| closure AST and recovery | focused modules under `ast` and `parser` |
| closure local/capture identity | `resolve/closures` |
| closure plans and contextual inference | `typecheck/closures` |
| structural callable contracts and direct-call validation | `typecheck/callables` |
| callable-call signature and specialization facts | `typecheck/facts/callables` |
| capture loans, moves, and cleanup | existing `typecheck/ownership` consuming closure plans |
| anonymous layout and static invocation | focused `ir/lower/closures` support |
| nested concrete cleanup reachability | `analysis/drop_dependencies` |
| editor-facing callable facts | compiler `analysis`; protocol conversion in `driver/lsp` |

## Verification

Focused tests cover required/default conformance, conformance-body override, generic and concrete lookup,
default ambiguity, method-generic scope, zero/one/two-parameter closures, every capture mode,
move-only cleanup, closure provenance and allocation effects, map/filter order, exact-size
preservation, terminal collection, region escape, incomplete editor input, and repository plus
packaged-home native execution.
