# nocter-compiler-computation

## Responsibility

Own the compiler-domain query entry shared by ephemeral command compilation and persistent
workspace analysis.

## Contract

The crate accepts atomic source revisions and returns an owner-bound revision token. That token is
required both to lend the computed syntax provider and to analyze a discovered unit. The crate
publishes the exact unit into semantic inputs and demands the sole complete-or-incomplete unit
analysis product. It owns the computation database and query instrumentation but does not select
packages, discover modules, translate session outcomes, build targets, or interpret editor
requests.

## Internal Responsibilities

- source-text, parse, declaration-surface, and module-surface queries
- stable body-input collection from declaration surfaces
- atomic semantic-scope publication
- closed unit-analysis demand
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
- The crate owns no filesystem topology, package acquisition, session, target, native, or protocol
  policy.
