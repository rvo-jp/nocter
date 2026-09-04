# nocter-compiler-computation

## Responsibility

Own the compiler-domain query entry shared by ephemeral command compilation and persistent
workspace analysis.

## Contract

The crate accepts atomic source revisions and returns an owner-bound revision token. That token is
required both to lend the computed syntax provider and to analyze a discovered unit. The crate
derives and publishes semantic inputs from that exact unit, then demands the sole
complete-or-incomplete unit analysis product. The publication types and semantic query entries are
private to this crate, so callers cannot assemble mismatched module, body, and scope authority. It
owns the computation database, retention policy, and query instrumentation but does not select
packages, discover modules, translate session outcomes, build targets, or interpret editor
requests.

## Internal Responsibilities

- content-addressed parse, declaration-surface, and module-surface queries
- stable body-input collection from declaration surfaces
- atomic semantic-scope publication
- private declaration, preparation, body, finalization, and unit query graph
- closed unit-analysis demand
- bounded retention of source revisions and dependency-closed inactive-entry collection
- execution and reuse statistics for equivalence tests

## Invariants

- Command and workspace callers differ only in owner lifetime, not query providers or stage order.
- Discovery supplies one already-ingested normalized source value to the parse query. The query key
  owns that exact text; compiler computation never reopens a source path.
- Declaration-surface queries consume the same content-addressed parse product. Module-surface
  queries consume declaration surfaces rather than raw source identity, so body-only edits do not
  invalidate module contracts.
- A source token from another owner or an earlier source revision is rejected before any query is
  supplied or semantic input is published.
- Semantic input publication may advance the internal database without invalidating the current
  source token; source authority and internal query revisions are separate identities.
- One discovered unit supplies both semantic and exact-current fingerprints atomically.
- Callers cannot access the raw computation database or demand an intermediate semantic query.
- Authored rejection and compiler-domain integrity failure are separate outcomes. A rejected stage
  prevents its downstream query from being demanded; projection or checking failure retains its
  typed cause through session.
- Complete, incomplete-syntax, declaration, preparation, body, and finalization queries validate
  their own input capability. Correctness does not rely on an upstream caller remembering the
  scheduler order.
- A body query receives a sealed exact-body input that binds the demanded physical source identity
  and fingerprint to the declaration identity or lexical product that consumes it. The checking
  context cannot accept an unrelated source token as a procedural invalidation proof.
- Incomplete declaration recovery retains both the authoritative declaration rejection and any
  later preparation or body-check rejection reached while collecting editor evidence.
- The crate schedules checking through `ReusableCheckingQuery`, but does not construct its internal
  join. Checking itself owns the paired declaration recipe and materializes the exact-current
  bindings, spellings, and source projection behind one safe transition.
- Source admission fingerprints an overlay without duplicating its bytes into the query database.
  Old semantic inputs and query products cannot accumulate beyond the retained source-revision
  window.
- The crate owns no filesystem topology, package acquisition, session, target, native, or protocol
  policy.
