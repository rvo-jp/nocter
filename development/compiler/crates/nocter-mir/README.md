# nocter-mir

## Responsibility

Lower one closed executable program into concrete target-independent semantic control flow.

## Contract

MIR consumes monomorphized items, selected operations, concrete representations, cleanup plans, and
runtime roles, and closed target-service descriptors. It publishes validated functions, immutable
static identities and frozen values, places, values, blocks, operations, packs, primitive
dependencies, and foreign-call plans. It does not inspect syntax, resolve names, prove requirements,
or assign a machine ABI.

## Internal Responsibilities

- CFG and dense local identity construction
- concrete place and projection lowering
- lossless transport of checking-owned safety dispositions
- canonical physical aggregate assembly after source-ordered initializer evaluation
- explicit cleanup, destruction, region, outcome, and switch edges
- deferred execution, suspension edges, continuation liveness, and checked cancellation plans
- the compiler-owned process operation that drives a selected deferred entry without changing
  ordinary call semantics
- operation and pack schemas
- whole-program MIR validation
- private activation requirements and lifecycle storage effects under `value_access`, separate
  from complete executable MIR bodies

## Invariants

- Each block has one exact terminator and typed merge contract.
- Struct initializer evaluation order remains checked-program order, while the finished MIR
  aggregate is ordered once by the executable runtime representation.
- Cleanup timing comes from checked plans, not operation-shape inference.
- Deferred cleanup distinguishes runtime-owned opaque storage from source aggregates. It retains
  the selected storage role and drop item without inventing fields for target-owned bytes.
- Calls target concrete executable item identities, closed primitive roles, or already validated
  target-service descriptors.
- Declared constants are resolved only through values frozen by executable reachability; MIR never
  reads declaration records or repeats constant evaluation.
- Validation checks representation integrity, not source-language acceptance.
- Dynamic indexes retain the exact bounds-check disposition selected by checking. MIR neither
  derives a proof from constants nor changes a required check into a proven result.
- Source arithmetic retains checking's exact trap disposition. Compiler-generated loop and pack
  arithmetic uses an explicit internal-invariant state instead of masquerading as source proof or
  silently selecting unchecked source semantics.
- Suspension-frame liveness is derived once from the closed MIR CFG. Cancellation order and
  conditional initialization are consumed from checked ownership rather than inferred from MIR
  operation shapes.
- A fallible deferred output completes as one ordinary fallible value; MIR does not invent a
  second task-failure channel.

## Private Storage Companion

`value_access::StorageProgram` consumes the complete private lifecycle product and its exact
instance data/input authority. Its builder interns dense slots, lowers resource phases and fixes
call-address bindings after commit-time backing moves. The frozen companion contains a runtime
environment, input signatures, storage requirements and projected effects, with no checking/target
storage reference. It is not a MirProgram and has no executable conversion. Full body lowering must
integrate it within one authority; raw local/location identities cannot pair unrelated generations.

The [private activation handoff](../../../architecture/pipeline/value-access-declarations.md#private-activation-storage-handoff)
owns cross-stage ordering and remaining execution obligations.
