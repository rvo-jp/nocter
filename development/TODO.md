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

The [v0.20.0 milestone](milestones/v0.20.0.md) completed its four compiler-foundation phases.
Phase 0 was reopened twice and completed
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

Phase 2 unified command and workspace analysis behind `nocter-compiler-computation` is complete
after its final enforcement review. The
[Phase 2 review](reviews/v0.20.0-phase-2.md) records the deleted eager session path, query-backed
package/discovery flow, native target boundary, parse-goal correction, post-commit package snapshot
fix, instrumentation, and qualification evidence.

Phase 3 dependency-local exact-selection migration is complete and reviewed. It removes top-level
`#lock` without compatibility parsing and makes each dependency record the sole syntax authority
for its source intent plus optional `commit` or `sha256`. Source intent and exact selection remain
separate domain values; package cache entries retain no selection authority. The
[Phase 3 review](reviews/v0.20.0-phase-3.md) records the final boundary audit and evidence.

## Next Work

The [v0.21.0 milestone](milestones/v0.21.0.md) is active. Phase 0 completed and reviewed the
separately accepted public design for representation-neutral `Map` and `Set`, equality/hash
coherence, keyed mapping literals, mutation and iteration guarantees, allocation failure, and the
minimum implementation prerequisites. Public behavior belongs only to
[Associative Collections](../spec/27-associative-collections.md); the cross-responsibility
implementation boundary belongs to
[Associative Collection Implementation Boundary](docs/associative-collection-implementation.md).

Phase 1 is complete and reviewed. The
[Phase 1 review](reviews/v0.21.0-phase-1.md) records the single keyed-pack authority, entry ABI,
separate key/value ownership, ordinary `u64` mixing API, narrow entropy adapter, editor behavior,
and qualification evidence.

Phase 2 is complete and reviewed. The
[Phase 2 review](reviews/v0.21.0-phase-2.md) records the opaque state, package-only lifecycle,
private keyed streaming algorithm, coherent scalar/text/view/owning implementations, and native
qualification.

Phase 3 is complete and reviewed. The
[Phase 3 review](reviews/v0.21.0-phase-3.md) records the dense ownership and bucket-metadata split,
failure-atomic capacity publication, package-only safety boundary, public `Map<K, V>` delegation,
and native qualification.

Phase 4 is complete and reviewed. The
[Phase 4 review](reviews/v0.21.0-phase-4.md) records package-only paired cursors,
readonly/readwrite/owning Map iteration, readonly/owning Set iteration, exact remaining lengths,
shared storage delegation, prelude exports, and native cleanup qualification.

Phase 5 is complete and reviewed. The
[Phase 5 review](reviews/v0.21.0-phase-5.md) records the runnable `line-frequency` package, exact
native process contract, public-source misuse diagnostics, shared checked editor queries,
mapping-literal formatter correction, generated documentation, and final private-representation
audit.

Begin v0.21.0 release preparation. Freeze the qualified language and standard-library surface;
update release identity and notes, assemble deterministic artifacts, run archive and installed
toolchain qualification, then publish only after explicit authorization. Do not add release-scope
features or introduce `HashMap`/`HashSet` aliases, a compiler-known table, two independent Map/Set
storage engines, or public bucket access.

Do not cache `NodeId`, `SourceId`, frontend bindings, or `SourceIndex` as if they were reusable
semantic programs. Stable declaration identities, module-local semantic queries, feature-demand
editor analysis, cancellation, parallelism, and persistent caches remain later work.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
