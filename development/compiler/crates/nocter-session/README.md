# nocter-session

## Responsibility

Orchestrate one compiler semantic pipeline from a closed compile input through target validation or
explicit recovery evidence.

## Contract

The crate accepts either a closed compile input for direct compilation or a closed outcome from the
semantic query graph. Direct compilation runs preparation, body checking, and target construction
in the only production order. Query-backed analysis only opens the supplied success or rejection
branch and publishes one immutable session outcome with capability-oriented semantic views for
complete programs, typed bodies, lexical names, typed interruptions, and exact repair authority.
Recovery storage variants and phase order remain private. The crate does not implement stage rules,
editor queries, native code generation, or protocol projection.

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
- Query-backed lexical rejection opens the finalization query's exact-current recovery branch.
  Session translates its typed diagnostic and recovery evidence without invoking name resolution,
  checking preparation, or body checking.
- No public query-continuation entry point accepts declarations, prepared programs, body-name sets,
  or typed-body sets. A source-complete query consumer must supply one closed final success or
  rejection branch.
- Query-backed incomplete syntax opens one exact-current analysis product and only translates its
  compiler-domain failure/evidence branch. The recovery traversal itself belongs to semantic
  computation; the direct session API delegates to the same traversal.
- Source-complete query analysis has one public consumer accepting the closed top-level product.
  Branch-specific consumers are private translation helpers, so callers cannot choose declaration,
  preparation, lexical, body, or finalization order.
