# Nocter Development Handoff

## Current State

Nocter v0.33.0 is [published and externally audited](releases/v0.33.0.md). Its structural-tuple
contract, exact source, reproducible artifact, publication, and public re-download evidence are
frozen. The `v0.33.0` tag and release asset must not be replaced.

The [v0.33.0 structural-tuple milestone](milestones/v0.33.0.md) is complete, published, and
externally audited. Tuples are anonymous ordered products across syntax, semantic identity,
ownership, cleanup, MIR, runtime shape, machine layout, native execution, formatting, and semantic
tooling. `str.split_once` and the public tuple example exercise the feature without replacing
meaningful named records. The exact replacement candidate passed two independent workspace and
Clippy runs, public-HTTPS acquisition, reproducible packaging, every public example, tuple-specific
native behavior, framed LSP verification, immutability checking, and tamper rejection. Its
immutable [publication record](releases/v0.33.0.md) owns the tag, public asset, and re-download
evidence.

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

The [v0.30.0 synchronous subprocess milestone](milestones/v0.30.0.md) is complete, published, and
externally audited. Its owning `Command`, exact launch behavior, synchronous child lifecycle,
unambiguous exec-failure reporting, and typed exit status pass native, editor, formatter,
complete-workspace, reproducible-package, installed-home, and public-asset qualification. Its
[Phase 5 review](reviews/v0.30.0-phase-5.md) has no open finding.

## Next Work

Define the next coherent practical or language boundary before changing compiler or standard-
library behavior. Preserve the v0.33.0 release record as immutable evidence; any correction
requires a new version, candidate, tag, and archive.

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
