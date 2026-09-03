# Nocter Development Handoff

## Current State

Nocter v0.31.0 is [published and externally audited](releases/v0.31.0.md). Its captured-subprocess
contract, exact source, reproducible artifact, publication, and public re-download evidence are
frozen. The `v0.31.0` tag and release asset must not be replaced.

The [v0.31.0 captured-output milestone](milestones/v0.31.0.md) is complete, published, and
externally audited. `Output` and consuming `Command.output` are implemented, and one private Darwin
lifecycle composes
normalized launch and capture pipes, exact descriptor installation, staged launch reports,
two-stream polling, owning byte buffers, and exact-child observation. Public and private native
qualification now covers empty and exact text output, arbitrary bytes, nonzero and signal
termination, setup and exec rejection, repeated cleanup, and initially closed parent standard
descriptors. The public `subprocess-output` package now crosses formatter, editor, native, command,
and installed-home boundaries through one shared execution contract. Complete workspace,
warnings-denied, no-default-features, documentation, packaging-input, and repository qualification
passes. Its [Phase 5 review](reviews/v0.31.0-phase-5.md) has no open finding. The exact candidate
commit passed two independent workspace and Clippy runs, reproducible packaging, installed-home
execution, framed LSP verification, and tamper rejection. Its immutable
[publication record](releases/v0.31.0.md) owns the tag, public asset, and re-download evidence.

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

Implement [v0.32.0](milestones/v0.32.0.md) as one complete configured synchronous-command release.
Phase 0 fixed the public contract and Darwin feasibility. Phase 1 now owns environment,
working-directory, and input configuration plus one prepared launch plan without changing the
compiler. Phase 2 must replace capture-only polling with one command-I/O session that composes
finite writes and both captured reads. Later phases qualify every configuration with both terminal
methods, integrate one public example, and close the entire practical use case before release
preparation.

Preserve the v0.31.0 release record as immutable evidence; any correction requires a new version,
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
