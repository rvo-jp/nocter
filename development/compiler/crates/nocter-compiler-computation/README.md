# nocter-compiler-computation

## Responsibility

Own the compiler-domain query entry shared by ephemeral command compilation and persistent
workspace analysis.

## Contract

The crate accepts atomic source revisions, lends one computed syntax provider to physical package
and discovery owners, publishes one exact discovered unit into semantic inputs, and demands the
sole complete-or-incomplete unit analysis product. It owns the computation database and query
instrumentation but does not select packages, discover modules, translate session outcomes, build
targets, or interpret editor requests.

## Internal Responsibilities

- source-text, parse, declaration-surface, and module-surface queries
- stable body-input collection from declaration surfaces
- atomic semantic-scope publication
- closed unit-analysis demand
- execution and reuse statistics for equivalence tests

## Invariants

- Command and workspace callers differ only in owner lifetime, not query providers or stage order.
- Source parsing used by discovery and declaration surfaces comes from the same query product.
- One discovered unit supplies both semantic and exact-current fingerprints atomically.
- Callers cannot access the raw computation database or demand an intermediate semantic query.
- The crate owns no filesystem topology, package acquisition, session, target, native, or protocol
  policy.
