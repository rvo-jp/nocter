# nocter-runtime-contract

## Responsibility

Own source-independent runtime primitive roles, canonical runtime representations, target runtime
requirements, and the closed environment passed toward native lowering.

## Contract

Declaration lowering projects authorized source declarations onto these roles. Target, MIR, and
machine stages consume the selected roles; they do not rediscover them by function name or standard
module path.

## Internal Responsibilities

- primitive role identities
- closed positive effect evidence for primitive roles
- canonical representation classes
- target runtime capability requirements
- closed runtime environment schemas

## Invariants

- A role has one numeric and structural authority.
- Source spelling and visibility are not runtime identities.
- Machine consumers cannot reach declaration or checking storage through this contract.
- Primitive effect facts are keyed by closed roles, never inferred from source names or target
  instruction sequences.
