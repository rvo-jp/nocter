# Nocter Development Handoff

## Current State

Nocter v0.19.0 is [published and externally audited](releases/v0.19.0.md). Phase 0 filesystem
traversal, Phase 1 streaming text input, Phase 2 collection ordering, Phase 3 recursive text search,
and Phase 4
implementation stabilization are complete and reviewed. Release-content commit
`3b510875b086513c5fdcde970628267630d7f5d0` passed duplicate source and artifact qualification.
Its public stream, UTF-8 and error policy, close-once ownership, bounded storage, move-only
ordering, native application behavior, and editor coverage define the frozen release. The exact
source, artifact, publication, and public re-download evidence is frozen. The `v0.19.0` tag and
release asset must not be replaced.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
`development/milestones/v0.19.0.md`, `development/releases/v0.19.0.md`, and
`development/reviews/` rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

The [v0.20.0 milestone](milestones/v0.20.0.md) is active. Phase 0 was reopened twice and completed
after design rework and full compiler review on 2026-08-29. It separates dependency and inheritance
graphs, expands direct prerequisites through a bounded predicate worklist, separates proof and body
requirements, and separates authored roots from exact capability evidence while retaining explicit
implementation semantics. Session recovery is exposed to editor queries through semantic
capabilities rather than phase selection. The
[Phase 0 review](reviews/v0.20.0-phase-0.md) records the corrected boundary audit and qualification
evidence.

Phase 1 incremental semantic computation is complete and reviewed. The
[Phase 1 review](reviews/v0.20.0-phase-1.md) records the closed query graph, source-authority
boundary, rejection and recovery model, instrumentation, and qualification evidence. Workspace
analysis now demands one complete or incomplete top-level semantic product; it cannot select an
earlier compiler phase or reconstruct a missing result.

## Next Work

Define v0.20.0 Phase 2 before implementing hashing or associative collections. The next design must
choose the stable `Hash` and `Hasher` contracts, deterministic-versus-randomized hashing policy,
allocator and recoverable-failure behavior, the relationship between hashing and key equality,
iteration-order guarantees, and the minimum `HashMap` and `HashSet` API. Phase 2 must consume Phase
0 prerequisite closure and Phase 1 query products without source-text inference, duplicate lookup,
or another semantic pipeline.

Do not cache `NodeId`, `SourceId`, frontend bindings, or `SourceIndex` as if they were reusable
semantic programs. Hashing and associative collections remain later v0.20.0 phases.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
