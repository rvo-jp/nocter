# Persistent Semantic Authority

This document defines the adopted v0.18.0 Phase 3 compiler contract. It does not define Nocter
source-language behavior. The milestone completion gate remains authoritative for phase status.

## Problem Boundary

Before Phase 3, body checking mutated the canonical type store, copyability table, and closure
builder. It captured three rollback boundaries before each body. Success committed copyability and
closure journals; failure cloned complete type and copyability state when member recovery needed
provisional facts, then rolled all three authorities back.

That rollback contract was negative: every future mutation had to remember to join the journal.
Recovery cost also scaled with the complete semantic store rather than the facts introduced by the
failed body. Moving only the type store would have retained the same failure mode in copyability
and closure construction.

## Adopted Authority Model

Prepared, checked, and recovery products own immutable semantic authorities. Body checking opens
one branch-local transaction containing three coordinated overlays:

```text
BodySemanticAuthority
    |
    `-- BodySemanticTransaction
            |-- TypeTransaction
            |-- CopyabilityTransaction
            `-- ClosureTransaction
```

The transaction reads its immutable base and owns only branch-local additions. Success consumes the
transaction into one descendant authority. Failure cannot modify the base: it discards the branch
or freezes that exact branch as a tooling capability.

No compiler consumer receives a persistent chunk, intern index, mutation journal, or lineage
implementation. The dependency-free `nocter-persistent` crate owns only path-copying collection
mechanics. `nocter-model` wraps those mechanics in type and semantic-ID authority, while
`nocter-checking` owns copyability, closure, and body-transaction policy. Read-only algorithms
consume an immutable `&TypeStore`; algorithms that may intern structural types receive a
`TypeTransaction`. The same separation applies to copyability and closure state. A second view
abstraction is unnecessary because `TypeStore` itself has no mutating API.

## Identity and Lineage

A type identity is meaningful only in its owning authority. Descendants preserve the complete
ancestor prefix, so checked bodies committed earlier remain valid in later sequential descendants.
Sibling branches are independent and cannot be merged or exchange bare branch-local identities.

Every component transaction records its exact base lineage. `BodySemanticTransaction` is the only
capability that exposes all three body branches and the only body-level commit boundary. Commit
consumes it and rejects a stale or foreign component base without mutating any accepted authority.
Recovery freezes the required branch together with its authority; editor APIs never separate a
provisional `TypeId` from that value. A self-contained `TypeProjection` remains the boundary for
isolated type presentation.

## Storage Contract

Fork and immutable snapshot are constant-time authority operations. Newly interned types and proof
facts occupy branch-local storage. Persistent storage may use immutable chunks and a structurally
shared index, but those choices are private and replaceable.

An `Arc<Vec<_>>` followed by copy-on-write mutation is not sufficient: it moves the complete clone
from snapshot creation to the next write. A layered linear lookup is also not a final design because
its cost grows with the number of committed bodies. The selected implementation must keep lookup
and interning bounded independently of body count and must preserve deterministic identity.

## Pipeline Contract

```text
prepared authority
    |
    | fork
    v
body transaction -- success --> commit descendant --> next body
    |
    `-- failure --> discard
              `-> freeze typed recovery
```

Source projection is published alongside a successful checked body, not before it. Independently
successful bodies may remain sparse editor evidence after another body fails, but no recovery value
can enter ownership, provenance, target closure, MIR, or backend construction as a checked program.

## Editor Contract

Construction, structural-field, enum-pattern, and associated-type interruptions retain only their
selected identities. Outcome repair retains one closed `TypeProjection`. Member selection retains
one frozen semantic branch because ordinary method selection needs provisional receiver types and
copyability facts.

Member completion opens a query transaction from that branch. Generation-local query state may
retain the query delta, but immutable compiler products remain unchanged and no complete store is
cloned on first use. Each query session verifies that both of its transactions were opened from the
exact type and copyability authorities supplied by the query. Reusing a session with another
compiler generation or another recovery interruption is an error rather than silent cross-branch
identity reuse.

## Qualification

The structural qualification compares authority operations rather than relying only on noisy wall
clock measurements:

| Operation | Before Phase 3 | Required after Phase 3 |
| --- | --- | --- |
| Body start | three checkpoints | one constant-time fork |
| Successful body | direct mutation plus journal commit | consume-and-commit descendant |
| Rejected body | three rollbacks | discard branch |
| Member recovery | complete type/copy clone | shared base plus body delta |
| First member query | complete type/copy clone | query overlay fork |

Warm success, multi-error recovery, and repeated-completion timings are recorded before final
qualification. A result above the milestone regression thresholds blocks completion until explained
and corrected.
