# nocter-semantic-computation

## Responsibility

Own stable semantic-scope inputs and the demand query graph from discovered source products to
source-neutral compiler results.

## Contract

Workspace orchestration publishes one immutable discovery snapshot through separate declaration-
semantic and exact-current-source fingerprints. Queries read the narrowest input required by their
outcome and publish reusable semantic products without exposing query storage or invalidation
policy. This crate does not discover files, interpret editor requests, or invoke target backends.

## Internal Responsibilities

- stable scope-key construction
- atomic publication of semantic and current-source views of one discovery snapshot
- declaration-query evaluation and dependency selection
- computation instrumentation for semantic query tests

## Invariants

- An accepted declaration result depends only on source-neutral topology and declaration surfaces.
- A rejected declaration result additionally depends on exact current source, so generation-local
  recovery can never survive an edit through an unchanged declaration fingerprint.
- Exact current-source identity covers canonical path, normalized bytes, and the current
  `SourceId` layout. A rejected query may therefore retain its diagnostic and recovery projection;
  session opens a cloned analysis branch instead of rerunning declaration lowering.
- Both input families retain the same immutable `DiscoveredUnit`; a query cannot join topology from
  one snapshot with source bytes from another.
- Compiler-domain rejection is a query value, not a computation-kernel failure.
- Query keys contain no `SourceId`, `NodeId`, semantic arena ID, or workspace generation number.
- Reusable declaration symbols form a stable prefix. Current body spellings are appended only to
  an owned checking branch, so body edits cannot renumber declaration symbols or enter the
  declaration query result.
