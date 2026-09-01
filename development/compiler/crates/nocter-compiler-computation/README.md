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

- source-text, parse, declaration-surface, and module-surface queries
- stable body-input collection from declaration surfaces
- atomic semantic-scope publication
- private declaration, preparation, body, finalization, and unit query graph
- closed unit-analysis demand
- bounded retention of source revisions and dependency-closed inactive-entry collection
- execution and reuse statistics for equivalence tests

## Invariants

- Command and workspace callers differ only in owner lifetime, not query providers or stage order.
- Source parsing used by discovery and declaration surfaces comes from the same query product.
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
- The exact-current checking join is constructed only inside this crate. Workspace, session, and
  protocol layers cannot independently pair reusable semantics, bindings, spellings, or source
  projection.
- Old overlay bytes, semantic inputs, and query products cannot accumulate beyond the retained
  source-revision window.
- The crate owns no filesystem topology, package acquisition, session, target, native, or protocol
  policy.
