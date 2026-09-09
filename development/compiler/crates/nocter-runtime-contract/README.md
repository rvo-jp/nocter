# nocter-runtime-contract

## Responsibility

Own source-independent runtime primitive roles, trusted typed import identities, canonical
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
- kind-preserving trusted function/data symbol identities and operating-system library identities,
  without concrete loader paths
- finite target-service roles, target identity, calling convention, and fixed foreign ABI classes
- closed compiler-owned storage roles and their ABI-specific layouts
- canonical representation classes
- fixed async owning-handle and heap-frame-header ABI
- the fixed Darwin Blocks ABI subset admitted by compiler-owned native adapters
- the fixed Darwin network callback-event record, payload ownership table, provider state codes,
  and two-step release fence consumed through a descriptor channel
- the closed Darwin network adapter import catalog, callback signatures, and complete-transfer
  classification
- closed async lifecycle-state tag sequences
- target-independent descriptor and monotonic-timer wait interests
- target runtime capability requirements
- closed runtime environment schemas

## Invariants

- A role has one numeric and structural authority.
- Source spelling and visibility are not runtime identities.
- A loader symbol is validated once, retains whether it denotes a function or data object, and
  cannot be selected from user source.
- A target-service binding owns its closed descriptor; later stages cannot reconstruct the
  descriptor from its role or declaration spelling.
- A runtime-storage role has one declaration binding and one ABI-selected layout; source cannot
  supply its fields, size, or alignment. Native owners and ownership-bearing callback events keep
  distinct roles even where their current layouts happen to have equal dimensions.
- Machine consumers cannot reach declaration or checking storage through this contract.
- Native callback layouts have one numeric schema; source code cannot construct or inspect them.
- The Darwin callback channel carries complete event records. A shared queue, lock, and wake-only
  signal are not part of the runtime contract.
- Network adapter consumers select typed catalog roles; they cannot reproduce loader symbols,
  symbol kinds, libraries, Block signatures, event size, or interrupted-transfer classification.
- A final Network.framework state must be followed by a barrier on the same serial dispatch queue;
  event receipt alone never authorizes native-owner release.
- Primitive effect facts are keyed by closed roles, never inferred from source names or target
  instruction sequences.
- A primitive role may immediately construct an opaque asynchronous value. This does not classify
  the primitive as a deferred Nocter body; the role, result contract, and target helper jointly
  define the construction boundary.
- Semantic descriptor/timer interests and their numeric ABI records have one mapping here. Reactors
  and target backends consume that mapping instead of assigning independent meanings to tags.
- A computation supplies only its suspension-state count. This contract assigns its initial,
  suspension, and completed tags; a target backend cannot repeat the arithmetic or assume zero as
  the initial encoding.
- Timer ordering is a half-domain wrapping-counter contract. A target wait-width cap may divide one
  deadline into multiple native waits, but cannot make the logical interest eligible early.
