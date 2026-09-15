# Nocter Development Handoff

## Current State

Nocter v0.53.0 is a qualified release candidate. The exact release-content commit passed the
whole-repository compiler gate, deterministic two-build packaging, and fresh installed-home
qualification. The retained archive and measured identities are recorded in the release-preparation
record. The completed implementation scope lives in
[`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Await explicit publication authorization. Publication must reuse the retained qualified archive
without rebuilding it, then update published-version surfaces and record an immutable release
audit. Do not tag, upload, or publish before that authorization.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
