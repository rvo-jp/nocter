# Callable Values and Contract-Derived Extensions

This document owns the compiler implementation design for v0.3.0 Phase 10. Public closure,
extension, and iterator-chain semantics belong to the specification. The active completion gate
belongs to the [v0.3.0 Development Contract](v0.3.0.md).

## Separation of Responsibilities

Phase 10 preserves three distinct composition mechanisms:

| Mechanism | Responsibility |
|---|---|
| interface | declares a capability contract and contains no reusable body or stored state |
| extension | implements behavior derivable from an existing interface contract |
| embedding | owns a stored component and participates in layout, provenance, and cleanup |

An extension never satisfies an interface requirement, changes a target type's layout, or grants
conformance. A closure environment is an ordinary owned compiler-generated aggregate; it is not an
extension and is not an interface object.

## Extension Declarations

The Phase 10 form is deliberately constrained:

```nct
extension<T, I: Iterator<T>> I {
    pub method self.map<U, F: CallMut<T, U>>(
        transform: F
    ): MapIter<T, U, I, F> {
        return MapIter.new(move self, move transform)
    }
}
```

- the target is one extension generic parameter
- that parameter has at least one interface bound
- every member is a body-bearing method; fields, drop members, and associated functions are invalid
- method generic parameters are scoped after extension generic parameters and may not reuse names
- the extension body may use only the target's public bound surface and ordinary public APIs
- a method becomes visible only through its declaration's imported module identity

Method lookup first considers an inherent or generic-bound contract member. If no such member
resolves, it considers imported extension declarations whose complete bound set is satisfied.
Multiple applicable extension declarations are an ambiguity diagnostic. Source or import order is
never a tie breaker.

Resolution records the selected extension declaration, specialized receiver, generic
substitutions, and ordinary callable target. Typecheck, ownership, allocation analysis,
buildability, IR, and LSP consume that fact; none rewrites a member spelling into a free function.

## Closure Syntax and Capture Ownership

A closure is a value expression:

```nct
(value) { value * 2 }
```

Its parameters may be inferred from an expected callable contract or annotated explicitly. A tail
expression produces the result. `return` exits the closure body, not the enclosing callable.

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
Phase 10 standard iterator adapters use mutable repeated call because an implementation may retain
state between elements. A closure may satisfy a stronger repeated capability only when its body
does not consume captured state.

## Shared Semantic Plan

Typecheck owns one immutable closure plan containing:

- closure declaration identity and anonymous type identity
- ordered capture places, modes, field types, and storage provenance
- specialized parameter and return types
- receiver capability and callable interface declaration identity
- body call target, allocation effect, and result provenance summary
- capture initialization, move, and cleanup obligations

Ownership consumes capture places and obligations from the plan. Provenance treats the closure as
an aggregate of capture fields and substitutes callable result origins through the generated
receiver and parameter identities. Allocation analysis follows the closure body call target in the
same fixed point as ordinary functions. IR materializes the capture aggregate once and calls the
generated target directly after specialization.

## Standard Iterator Chain

Focused modules own adapter state and their corresponding extension methods:

- `std/iter/map` owns `MapIter` and `map`
- `std/iter/filter` owns `FilterIter` and `filter`
- the existing range, chain, enumerate, and terminal modules expose their operations as extensions
- `std/iter/collect` owns consuming `to_vec`

`map` preserves exact size when its source is exact. `filter` never claims exact size because the
predicate controls cardinality. Every adapter owns its source and callback, allocates nothing, and
drops current state plus remaining source exactly once. `to_vec` uses the current aborting
allocation context through the existing vector builder.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| extension and closure AST | focused modules under `ast` |
| syntax and incomplete recovery | focused modules under `parser` |
| extension visibility and declaration identity | `resolve/extensions` |
| closure local/capture identity | `resolve/closures` |
| extension applicability and specialization | `typecheck/extensions` |
| closure plans and contextual signature inference | `typecheck/closures` |
| capture loans, moves, and cleanup | existing `typecheck/ownership` consuming closure plans |
| closure provenance and allocation effects | existing shared callable-summary machinery |
| anonymous environment layout and static invocation | focused `ir/lower/closures` support |
| editor-facing extension and closure facts | compiler `analysis`; protocol conversion in `driver/lsp` |

## Verification

Focused tests cover syntax and formatting, method-generic scope, extension import and ambiguity,
generic and concrete receivers, zero/one/two-parameter closures, every capture mode, move-only
cleanup, closure result provenance, callback allocation effects, map/filter order, exact-size
preservation, terminal collection, region escape, incomplete editor input, and repository plus
packaged-home native execution.
