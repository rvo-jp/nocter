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
- content-addressed parse queries and computation instrumentation

## Invariants

- Every document has exactly one selected or rejected topology result per revision.
- Topology is computed once and cannot vary between documents in the same revision.
- Compilation receives the exact package-root catalog that topology used; it cannot repeat root
  selection against the overlay.
- Ambiguous shared-source contexts are rejected rather than ordered.
- Changed/closed files may invalidate demand but cannot create demand.
- Foreign, cloned, or non-increasing revisions cannot mutate latest state.
- Reused parse products are rebound through the syntax-owned text/identity contract; workspace code
  cannot rewrite syntax identities itself.
