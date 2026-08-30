# Checked Program Boundary

This document owns the cross-crate contract from accepted declarations through finalized checked
semantics. Public behavior remains in `spec/`; checking's private mechanisms belong in the
[`nocter-checking` README](../compiler/crates/nocter-checking/README.md).

## Boundary

```text
ReusableDeclarations
        |
        v
ReusablePreparedProgram
  + current declaration/body projection
        |
        v
independent lexical and typed-body query products
        |
        v
canonical whole-program finalization
        |
        +--> CheckedProgramOutput
        `--> typed authored rejection or integrity unavailability
```

Accepted declarations are the only semantic authority that may enter successful checking. Program
preparation validates and freezes declaration-wide types, copyability, interfaces, construction,
destruction, standard roles, and capability prerequisites before any body query runs.

`CheckedProgram` is syntax-independent executable semantics. A checked node may name semantic
identities, types, places, selected operations, dispatch, ownership, provenance, regions, and
cleanup decisions. It cannot contain syntax nodes, source ranges, documentation text, rendered
names, or reverse source-lookup keys.

## Stable Program and Current Source

Accepted declaration semantics retain a source-neutral projection recipe. Opening a current
checking generation materializes frontend bindings, source projection, body imports, and the exact
body-only symbol suffix from current syntax. It preserves the stable declaration symbol prefix and
semantic identity domain.

Program-wide preparation contains no generation-local source access. A current body query pairs
that immutable prefix with source access from its own admitted generation. Stable semantic
authority and current syntax therefore cannot be mixed across revisions.

## Decisions Crossing the Boundary

Checking is the sole owner of:

- lexical bindings and body-local identities;
- inferred and expected types;
- selected calls, members, operators, coercions, construction, and iteration;
- interface and structural requirement evidence;
- ownership, moves, loans, provenance, regions, cleanup, and destruction timing;
- abstract dispatch retained for later concrete specialization; and
- the exact authored reason a declaration, name, or body domain was rejected.

Target and MIR receive these decisions through semantic identities and closed plans. They cannot
receive syntax, a candidate set, a resolver, a rendered method name, or a requirement graph from
which the same decision could be repeated.

Structural capability evidence records the authored root, exact prerequisite derivation, and
specialized predicate once. Provenance, loans, concrete specialization, and editor queries consume
that record; none may reopen the root declaration to reconstruct proof.

## Independent Body Queries

Every body opens from the same prepared semantic prefix and an empty body-local type and closure
domain. One body cannot observe allocation order, inferred structures, closures, or memoized facts
created by a sibling.

An accepted lexical or typed body returns a source-neutral recipe with body-local identities.
Finalization replays the complete accepted set in canonical `BodyId` order, maps local identities
into the final program authority, and runs whole-program ownership, provenance, loans, opaque
witnesses, and semantic completion once. No consumer can pair a recipe with a separately supplied
body identity or invoke finalization a second time.

## Recovery Contract

Production success returns one complete checked program. Authored rejection is a different typed
query value. Every reached name and body domain is accepted or rejected explicitly, and each
rejection retains its source diagnostic and only the recovery capability justified by it.

A rejected branch cannot commit semantic mutations. Independently successful siblings may be
replayed for editor evidence, but they cannot turn incomplete coverage into compilation success.
An internal inconsistency is `Unavailable`; it cannot construct an authored diagnostic or trigger
an eager fallback through session.

## Source Projection

Current materialization and body replay extend one source projection beside semantic output. The
projection owns exact declaration, local, capture, checked-node, and reference occurrences. It is
not an input to lookup, visibility, typing, dispatch, ownership, provenance, or capability proof.

A missing or mismatched locator is an integrity failure for the joined generation. It does not
permit a guessed range or a partial editor result.

## Required Invariants

- Prepared semantic authority and current source access belong to one admitted compile scope.
- Every body-local ID belongs to exactly one body recipe.
- Every selected operation has one frozen semantic identity or plan.
- Proof-only declaration requirements cannot be used as body dispatch evidence.
- A successful body commits all local semantic changes together or publishes nothing.
- Canonical replay is the sole allocator of final body-added type and closure identities.
- A rejected body exposes only explicit recovery evidence from its own branch.
- Later stages cannot reopen checking selectors.
- Source projection cannot change semantic success.
