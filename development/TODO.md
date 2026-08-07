# Nocter Development Handoff

## Current Task

v0.8.0 Phase 0 borrowed-view coercion is complete. Stop at this boundary. Do not begin Phase 1
without a separate plan accepted by the project owner.

## Completed Checkpoint

- source-level result allocation modifiers and callable allocation variance were removed
- `from` is the sole public result-storage relationship and names only caller-managed external
  origins
- public body validation rejects an undeclared external result origin; private body inference stays
  exact without creating public syntax
- fresh result storage remains compiler-owned and propagates through outcomes, aggregates,
  callbacks, generics, interfaces, iterators, retained mutation, and ownership transfer
- unknown bodyless storage-bearing results use type-directed conservative internal storage
- the distributed standard library, formatter, AST JSON, normalized notation, and every LSP surface
  use the same source contract
- public specification pages and compiler-development documentation were migrated to the
  compiler-owned result-storage model
- missing-`from` validation covers every externally callable body form, and interface conformance
  cannot introduce an external result origin absent from its contract
- typed sequence literal packs can be named by `from items`, with fixed and spread element origins
  instantiated from declaration identity
- clean and incremental `scripts/verify.sh` runs each passed all 3,284 tests, formatting,
  warnings-denied Clippy, public examples, source corpus, and the distributed installed-home suite
- the 3,285,691-byte `arm64-darwin` archive with SHA-256
  `080160481adbcb0b7f64ab87903b05814aad13fc16207dcc9602e655675f2d78` passed the complete fresh
  extraction smoke matrix
- annotated tag `v0.7.0` resolves to publication commit
  `966c4a3e398ae534ad84ca5c8a35ae5ff0fcfdc8`
- the public GitHub Release contains exactly the qualified archive and is neither a draft nor a
  prerelease
- a separate public download reproduced the qualified size and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks
- v0.8.0 Phase 0 adds type-owned `coerce` declarations using `as`, coherent resolver identities,
  one expected-type `CoercionPlan`, ownership/provenance/region/IR integration, standard-library
  `String` and `Vec<T>` entries, and normalized editor presentation
- typed bindings, simple assignment, callable arguments, struct fields, fixed-array elements, and
  callable returns all record and lower through the same coercion-plan path
- `development/compiler/scripts/verify.sh` passed all 3,310 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 121 pages

The published v0.7.0 qualification remains in the immutable
[`releases/v0.7.0.md`](releases/v0.7.0.md) record. The implemented v0.8.0 Phase 0 contract and
completion evidence are recorded in [`milestones/v0.8.0.md`](milestones/v0.8.0.md).

## Next Work

Define v0.8.0 Phase 1 scope before making further language changes. Phase 0 deliberately leaves
owned, fallible, optional, allocation-backed, chained, inference-driven, member-lookup, and general
explicit conversion behavior out of scope.
