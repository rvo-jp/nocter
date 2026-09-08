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
- explicit cleanup, destruction, region, outcome, and switch edges
- deferred execution, suspension edges, continuation liveness, and checked cancellation plans
- operation and pack schemas
- whole-program MIR validation

## Invariants

- Each block has one exact terminator and typed merge contract.
- Cleanup timing comes from checked plans, not operation-shape inference.
- Calls target concrete executable item identities, closed primitive roles, or already validated
  target-service descriptors.
- Validation checks representation integrity, not source-language acceptance.
- Suspension-frame liveness is derived once from the closed MIR CFG. Cancellation order and
  conditional initialization are consumed from checked ownership rather than inferred from MIR
  operation shapes.
- A fallible deferred output completes as one ordinary fallible value; MIR does not invent a
  second task-failure channel.
