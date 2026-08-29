# nocter-session

## Responsibility

Orchestrate one compiler semantic pipeline from a closed compile input through target validation or
explicit recovery evidence.

## Contract

The crate accepts either a closed compile input or the accepted source-neutral result of the
declaration query, then runs preparation, body checking, and target construction in the only
production order. It publishes one immutable session outcome and capability-oriented semantic
views for complete programs, typed bodies, lexical names, typed interruptions, and exact repair
authority. Recovery storage variants and phase order remain private. The crate does not implement
stage rules, editor queries, native code generation, or protocol projection.

## Internal Responsibilities

- production and recovery semantic pipeline composition
- current projection materialization and checking continuation from reusable declarations
- complete diagnostic retention
- target and executable request composition
- semantic evidence handoff
- inseparable discovery-snapshot and session-outcome publication
- recovery-storage to query-capability projection
- session profiling and test selection

## Invariants

- Production and recovery cannot choose different semantic stage functions.
- Successful and recovered evidence are exclusive variants.
- A later failure cannot expose an older successful program.
- Failure-specific repair evidence moves once into its typed recovery owner.
- Consumers cannot select raw recovery phases or reconstruct phase fallback order.
- An analyzed unit cannot pair semantic evidence with a different discovery snapshot.
- The analyzed unit shares one immutable discovery snapshot with computation inputs; it never
  clones, rebuilds, or substitutes the source graph after a semantic query has consumed it.
- Query-backed checking creates an owned branch of the accepted declaration authority and
  materializes current frontend bindings plus source projection from its paired recipe. It then
  appends a deterministic current-body symbol suffix while preserving every reusable declaration
  symbol ID; it cannot invoke declaration lowering again.
- Query-backed declaration rejection opens an owned branch of the query's exact-current recovery
  product. Session composes its diagnostics and editor evidence without invoking declaration
  lowering a second time.
