# nocter-mir

## Responsibility

Lower one closed executable program into concrete target-independent semantic control flow.

## Contract

MIR consumes monomorphized items, selected operations, concrete representations, cleanup plans, and
runtime roles. It publishes validated functions, places, values, blocks, operations, packs, and
primitive dependencies. It does not inspect syntax, resolve names, prove requirements, or assign a
machine ABI.

## Internal Responsibilities

- CFG and dense local identity construction
- concrete place and projection lowering
- explicit cleanup, destruction, region, outcome, and switch edges
- operation and pack schemas
- whole-program MIR validation

## Invariants

- Each block has one exact terminator and typed merge contract.
- Cleanup timing comes from checked plans, not operation-shape inference.
- Calls target concrete executable item identities.
- Validation checks representation integrity, not source-language acceptance.
