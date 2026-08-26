# Persistent Semantic Authority

This document defines the adopted v0.18.0 Phase 3 compiler contract. It does not define Nocter
source-language behavior. The milestone completion gate remains authoritative for phase status.

## Problem Boundary

Body checking currently mutates the canonical type store, copyability table, and closure builder.
It captures three rollback boundaries before each body. Success commits the copyability and closure
journals; failure clones complete type and copyability state when member recovery needs provisional
facts, then rolls all three authorities back.

The rollback implementation is internally consistent, but its contract is negative: every future
mutation must remember to join the journal. Recovery cost also scales with the complete semantic
store rather than the facts introduced by the failed body. Moving only the type store would retain
the same failure mode in copyability and closure construction.

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
implementation. Read-only algorithms consume a `TypeView`; algorithms that may intern structural
types receive a `TypeTransaction`. The same separation applies to copyability and closure state.

## Identity and Lineage

A type identity is meaningful only in its owning authority. Descendants preserve the complete
ancestor prefix, so checked bodies committed earlier remain valid in later sequential descendants.
Sibling branches are independent and cannot be merged or exchange bare branch-local identities.

Every transaction records its exact base lineage. Commit consumes the transaction and rejects a
stale or foreign base. Recovery freezes the branch together with its authority; editor APIs never
separate a provisional `TypeId` from that value. A self-contained `TypeProjection` remains the
boundary for isolated type presentation.

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
cloned on first use.

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
