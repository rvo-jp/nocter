# Checked Program Boundary

This document owns the cross-crate contract from accepted declarations through checked semantics.
Public behavior remains in `spec/`; checking internals belong in the
[`nocter-checking` README](../compiler/crates/nocter-checking/README.md).

## Boundary

```text
AcceptedDeclarationProgram
  + immutable syntax snapshots
  + DiagnosticOrigins
  + semantic construction authority
        |
        v
CheckedProgram or typed recovery evidence
  + extended SourceIndex as a separate projection
```

`AcceptedDeclarationProgram` is the only declaration product that may enter successful checking.
The checker consumes it exactly once. Stable declaration, operation, destruction, standard-role,
and source-access facts move through one `ProgramEnvironment`; accepted type, copyability, and
closure facts move through one checked semantic authority.

`CheckedProgram` is syntax-independent executable semantics. A checked node may name semantic
identities, types, places, dispatch, ownership, provenance, region, and cleanup decisions. It cannot
contain syntax nodes, source ranges, rendered names, or reverse lookup keys.

## Decisions Crossing the Boundary

Checking is the sole owner of:

- lexical bindings and body-local identities;
- inferred and expected types;
- selected calls, members, operators, coercions, construction, and iteration;
- interface and structural requirement evidence;
- ownership, moves, loans, provenance, regions, and cleanup timing;
- abstract dispatch retained for later concrete specialization;
- the reason a name or body domain was rejected during editor recovery.

Target and MIR receive these decisions through checked identities and closed plans. They cannot
receive a syntax tree, resolver, candidate list, method spelling, or requirement set from which the
same decision could be repeated.

## Recovery Contract

Production success returns one complete `CheckedProgram`; there is no public partial checked
program. Editor recovery is a different typed result. Every reached body/name domain is explicitly
accepted or rejected, and each rejection retains the authored diagnostic or incomplete-syntax
reason that justifies absent facts.

An internal inconsistency cannot construct source-semantic recovery. A failed body branch cannot
mutate accepted semantic authority. Queryable interruption evidence remains bound to its exact
generation, body, source, and immutable semantic base.

## Source Projection

Checking may extend a `SourceIndex` builder with exact local, capture, checked-node, and reference
origins. The finished projection travels beside semantic output. Checking cannot read it as type,
lookup, visibility, dispatch, ownership, or provenance evidence.

## Required Invariants

- Stable program facts and semantic authority cannot be paired across compile units.
- Every body-local ID belongs to exactly one body.
- Every selected operation has one frozen semantic identity or plan.
- A successful body commits all type/copyability/closure changes together.
- A rejected body exposes only explicit recovery evidence from its own branch.
- Later stages cannot reopen checking selectors.
- Source projection failure cannot change semantic success.
