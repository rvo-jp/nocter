# Nocter Development Handoff

## Current State

Nocter v0.32.0 is [published and externally audited](releases/v0.32.0.md). Its configured-subprocess
contract, exact source, reproducible artifact, publication, and public re-download evidence are
frozen. The `v0.32.0` tag and release asset must not be replaced.

The [v0.32.0 configured-subprocess milestone](milestones/v0.32.0.md) is complete, published, and
externally audited. One owning `Command` composes arguments, exact environment edits, working
directory, and finite input with inherited-output `status` or captured `output`. One prepared
launch plan and one fair three-direction I/O session preserve exact-child ownership, bounded
progress, and deterministic failure precedence. The exact candidate commit passed two independent
workspace and Clippy runs, explicit public-HTTPS acquisition, reproducible packaging, every public
example and exact process contract, framed LSP verification, immutability checking, and tamper
rejection. Its immutable [publication record](releases/v0.32.0.md) owns the tag, public asset, and
re-download evidence.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The [v0.33.0 structural-tuple milestone](milestones/v0.33.0.md) is active. Phases 0 through 3
implement tuples as anonymous ordered products across syntax, semantic identity, checked ownership,
MIR, runtime shape, machine layout and lowering, formatting, semantic tooling, `str.split_once`, and
one public example. Their
[implementation review](reviews/v0.33.0-phases-1-3.md) has no open finding. The
[representation boundary](docs/tuple-design.md) prohibits synthetic nominal types, `PackEntry`
reuse, backend shape reconstruction, and editor-specific parsing.

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

v0.33.0 is qualified for publication. Its
[release-preparation record](milestones/v0.33.0-release-preparation.md) fixes candidate commit
`9fedd9a4be12d748f055da777436295021a4466a`, and the
[Phase 4 review](reviews/v0.33.0-phase-4.md) has no open finding. Do not rebuild the retained archive,
tag, push, upload, or advance public latest-release links without separate publication
authorization.
Preserve the v0.32.0 release record as immutable evidence; any correction requires a new version,
candidate, tag, and archive.

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
