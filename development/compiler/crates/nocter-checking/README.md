# nocter-checking

## Responsibility

Consume one accepted declaration program and produce syntax-independent typed semantics, ownership
facts, dispatch decisions, and explicit source-justified recovery evidence.

## Contract

Checking receives immutable program facts, exact body syntax projections, diagnostic origins, and
one semantic construction authority. A successful result exposes `CheckedProgram` and immutable
semantic authority. A rejected analysis result classifies every reached name/body domain and may
expose only the recovery capabilities justified by its diagnostics. Source projection is extended
beside, never inside, semantic output.

## Internal Responsibilities

- program-wide preparation and standard semantic roles
- lexical name evidence and body scopes
- type checking, inference, operations, construction, and calls
- interface implementation and instance-operation selection
- specialized interface-capability evidence and prerequisite validation
- ownership, loans, provenance, regions, cleanup, and destruction
- persistent type/copyability/closure transactions
- checked and recovery semantic queries

## Invariants

- One `ProgramEnvironment` carries stable facts through the complete checking lifetime.
- Declaration proof requirements cannot carry runtime evidence. Body requirements always carry
  one evidence identity; no optional-evidence state exists.
- One independent capability-evidence table owns each authored root, exact prerequisite origin,
  and specialized predicate consumed by checking descendants.
- Type and copyability authority cannot be paired across generations.
- A body transaction commits all semantic mutations together or is discarded/frozen as one branch.
- Checked dispatch is selected once; Target and MIR receive no lookup inputs.
- Generic lookup, provenance, loans, concrete dispatch, and editor signature queries consume the
  same frozen capability-evidence identity; a later stage cannot reinterpret the authored root
  requirement as a different predicate.
- A checked query derives type and visibility from its own body generation.
- `SourceIndex` cannot affect a semantic decision.

The [checked-program boundary](../../../docs/checked-program-design.md) documents contracts shared
with adjacent stages. The
[persistent authority record](../../../reviews/v0.18.0-persistent-semantic-authority.md) explains the
completed v0.18.0 migration.
