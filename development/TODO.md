# Nocter Development Handoff

## Current State

Nocter v0.24.0 is [published and externally audited](releases/v0.24.0.md). Its exact source,
artifact, publication, and public re-download evidence is frozen. The `v0.24.0` tag and release
asset must not be replaced.

The [v0.25.0 implementation](milestones/v0.25.0.md) is complete and qualified. It adds the sole
source-visible guarantee `noalloc`, backed by one checker-owned positive allocation-effect table,
closed standard and primitive effect evidence, deliberate standard-library contracts, and shared
tooling presentation. [Release preparation](milestones/v0.25.0-release-preparation.md) has qualified
the exact release-content commit, two independent source builds, one reproducible optimized
archive, and the isolated installed-home matrix. Publication has not been authorized.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

The [v0.25.0 architecture follow-up](reviews/v0.25.0-architecture-follow-up.md) closes the remaining
recovery-product reconstruction, executable-specialization lineage, duplicate topology-builder,
and development-catalog authority gaps found by the full-workspace review.

The reopened [query authority review](reviews/v0.25.0-query-authority-closure.md) replaces
policy-restricted checking friend APIs with checking-owned transition products and removes source
projection from target validation. Query correctness now follows from ownership and crate
dependencies rather than source-text assertions or Clippy method bans. Its final closure also
retains typed incomplete-declaration failures and seals exact body source input inside the
checking-owned context.

## Next Work

v0.25.0 is ready for an explicitly authorized publication operation. Reuse the retained qualified
archive without rebuilding it; advance public latest-release links, create and push the annotated
tag, upload exactly that archive, and verify the public download only after authorization. Do not
replace an existing tag or asset.

Do not add another source-visible guarantee merely because the internal effect representation can
express it. `notrap`, `noblock`, `nosuspend`, `realtime`, and a general effect list remain deferred
until a concrete user-facing contract needs them.

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
