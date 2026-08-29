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
- stable path-and-declaration-locator body input publication
- declaration-query evaluation and dependency selection
- reusable program-wide checking preparation above accepted declarations
- per-body lexical and typed queries sharing one exact-current semantic context
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
- Body source inputs use a canonical physical path plus a declaration-surface block locator; their
  fingerprints contain only that body's exact normalized bytes.
- Reusable declaration symbols form a stable prefix. Current body spellings are appended only to
  an owned checking branch, so body edits cannot renumber declaration symbols or enter the
  declaration query result.
- Successful program preparation owns only source-neutral environment and semantic authorities.
  Body-only edits reuse that query; current source access and body symbols join later through the
  checking-owned current-generation opening contract.
- One private current body-semantic context refreshes projection, symbols, and source access once per
  revision. Its semantic fingerprint is the declaration authority, so unchanged source-neutral
  body results remain reusable; a changed body reads the refreshed context through its own exact
  body input.
- A typed-body query depends on its lexical query and exact body input. It publishes one checked
  graph, body-local type/closure extensions, and body-local source projection recipe. The complete
  set is ordered by `BodyId`; workspace policy cannot observe or select query execution order.
- Authored typed rejection is retained under the exact-current fingerprint rather than collapsed
  into absence. Internal checking failure remains unavailable and cannot masquerade as source.
