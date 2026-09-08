# nocter-runtime-contract

## Responsibility

Own source-independent runtime primitive roles, trusted function-import identities, canonical
runtime representations, target runtime requirements, and the closed environment passed toward
native lowering.

## Contract

Declaration lowering projects authorized source declarations onto runtime contracts. Target, MIR,
machine, and executable stages consume selected identities; they do not rediscover them by function
name or standard module path. Loader symbols in this crate are validated target-catalog values, not
public source-level imports.

## Internal Responsibilities

- primitive role identities
- closed positive effect evidence for primitive roles
- trusted operating-system library and loader-symbol identities
- finite target-service roles, target identity, calling convention, and fixed foreign ABI classes
- canonical representation classes
- fixed async owning-handle and heap-frame-header ABI
- target-independent descriptor and monotonic-timer wait interests
- target runtime capability requirements
- closed runtime environment schemas

## Invariants

- A role has one numeric and structural authority.
- Source spelling and visibility are not runtime identities.
- A loader symbol is validated once and cannot be selected from user source.
- A target-service binding owns its closed descriptor; later stages cannot reconstruct the
  descriptor from its role or declaration spelling.
- Machine consumers cannot reach declaration or checking storage through this contract.
- Primitive effect facts are keyed by closed roles, never inferred from source names or target
  instruction sequences.
- A primitive role may immediately construct an opaque asynchronous value. This does not classify
  the primitive as a deferred Nocter body; the role, result contract, and target helper jointly
  define the construction boundary.
- Semantic descriptor/timer interests and their numeric ABI records have one mapping here. Reactors
  and target backends consume that mapping instead of assigning independent meanings to tags.
