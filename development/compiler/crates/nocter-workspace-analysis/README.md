# nocter-workspace-analysis

## Responsibility

Turn accepted document revisions into one frozen workspace topology, compilation demand, and set of
immutable analysis generations.

## Contract

The crate consumes canonical workspace roots, installation facts, and linear
`WorkspaceSourceRevision` values. It prepares package, toolchain-standard, or single-file scopes,
owns the revisioned computation database, supplies reusable source syntax to discovery, invokes
compiler analysis once per demanded scope, and publishes normalized workspace outcomes to the
language server.

## Internal Responsibilities

- revision causality and owner-sequence validation
- package-root catalog construction and topology freeze
- complete module-root compilation input construction
- scope caching, invalidation, and generation publication
- path-keyed source-text and parse queries, input publication, and computation instrumentation
- source declaration-surface and module-surface query composition
- atomic semantic-scope input publication plus closed semantic-outcome query demand
- exact per-body input collection from the already demanded source-surface product
- transport of only final complete/rejected semantic products to session

## Invariants

- Every document has exactly one selected or rejected topology result per revision.
- Topology is computed once and cannot vary between documents in the same revision.
- Compilation receives the exact package-root catalog that topology used; it cannot repeat root
  selection against the overlay.
- Ambiguous shared-source contexts are rejected rather than ordered.
- Changed/closed files may invalidate demand but cannot create demand.
- Foreign, cloned, or non-increasing revisions cannot mutate latest state.
- Overlay membership, each open source's bytes, and the filesystem epoch are separate inputs;
  editing one open source cannot dirty unrelated source-text queries.
- Reused parse products are rebound through the syntax-owned text/identity contract; workspace code
  cannot rewrite syntax identities itself.
- Speculative mutation validation owns an isolated computation database and cannot read or mutate
  the accepted revision's query state.
- A module surface depends on each member source's canonical declaration syntax. A body-only edit
  may reevaluate that source boundary, but its unchanged fingerprint prevents module recomputation.
- Workspace orchestration publishes semantic and exact-current fingerprints from the same shared
  discovery snapshot. It cannot inspect declaration storage, body recipes, or invalidation rules.
- For source-complete input, workspace passes only a closed semantic success/rejection product to
  session. Missing declaration, preparation, or finalization products are integrity errors; they
  cannot select a session fallback that reruns compiler stages.
- Workspace demands one top-level source-complete analysis query rather than declaration,
  preparation, lexical, typed, and finalization products separately. Its only semantic branch is
  complete versus incomplete syntax; the compiler query owns every later stage decision.
- For syntax-invalid input, workspace demands one exact-current incomplete-analysis product.
  Session opens that product only after validating its discovery snapshot; workspace cannot invoke
  or order declaration, preparation, or body recovery stages itself.
- Workspace neither enumerates body queries nor transports intermediate lexical or typed sets.
  Semantic computation owns complete-set demand and canonical finalization order.
