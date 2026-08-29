# nocter-session

## Responsibility

Orchestrate one compiler semantic pipeline from a closed compile input through target validation or
explicit recovery evidence.

## Contract

The crate calls declaration lowering, preparation, body checking, and target construction in the
only production order. It publishes one immutable session outcome and capability-oriented semantic
views for complete programs, typed bodies, lexical names, typed interruptions, and exact repair
authority. Recovery storage variants and phase order remain private. The crate does not implement
stage rules, editor queries, native code generation, or protocol projection.

## Internal Responsibilities

- production and recovery semantic pipeline composition
- complete diagnostic retention
- target and executable request composition
- semantic evidence handoff
- recovery-storage to query-capability projection
- session profiling and test selection

## Invariants

- Production and recovery cannot choose different semantic stage functions.
- Successful and recovered evidence are exclusive variants.
- A later failure cannot expose an older successful program.
- Failure-specific repair evidence moves once into its typed recovery owner.
- Consumers cannot select raw recovery phases or reconstruct phase fallback order.
