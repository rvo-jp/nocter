# Nocter Development Handoff

## Current State

Nocter v0.56.0 release preparation is active. The unified value-provenance, lending API, and
zero-copy standard-library milestone is complete, including its reopened presentation review and
whole-repository compiler gate. Release identity now selects `0.56.0`; public notes and the
release-preparation contract are authored. The completed scope lives in
[`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Commit the exact release content, run deterministic two-build packaging and installed-home
qualification from that clean commit, then record the measured artifact identities. Publication
must reuse the retained candidate archive without rebuilding it.

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
