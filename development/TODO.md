# Nocter Development Handoff

## Current State

v0.7.0 Phase 0 was completed on 2026-08-07. No later v0.7.0 phase is active. The released baseline
remains v0.6.0; Phase 0 has not been packaged or published as a v0.7.0 release.

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
- clean and incremental `scripts/verify.sh` runs each passed all 3,274 tests, formatting,
  warnings-denied Clippy, public examples, source corpus, and the distributed installed-home suite

The detailed design, non-goals, completion gate, and exact verification counts are recorded in
[`milestones/v0.7.0.md`](milestones/v0.7.0.md).

## Next Work

Define and adopt the next v0.7.0 phase before changing language behavior. Do not reintroduce an
allocation result modifier or infer an external `from` origin from a callable's parameter count.
Treat v0.6.0 release records, tags, and assets as immutable.
