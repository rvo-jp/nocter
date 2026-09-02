# Nocter Development Handoff

## Current State

Nocter v0.27.0 is [published and externally audited](releases/v0.27.0.md). Its filesystem and path
contract, exact source, reproducible artifact, publication, and public re-download evidence are
frozen. The `v0.27.0` tag and release asset must not be replaced.

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

The [v0.27.0 filesystem and path milestone](milestones/v0.27.0.md) is complete, published, and
externally audited. Allocation-free lexical path views and explicit directory lifecycle operations
pass native, editor, formatter, architecture, complete-workspace, reproducible-package,
installed-home, and public-asset qualification. Its [final review](reviews/v0.27.0-phase-4.md) has
no open finding.

## Next Work

Continue [v0.28.0](milestones/v0.28.0.md) from its completed tooling and practical-integration
phase. Phase 5 qualifies the complete release boundary through the workspace, warnings-denied
Clippy, formatter, documentation, native, reproducible-package, installed-home, and repository
gates. Its review must check for duplicate UTF-8 and scalar-formatting authorities, public internal
helpers, failure-policy conflation, hidden allocation or buffering, target leakage, obsolete
wrappers, and caller-discipline assumptions.

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
