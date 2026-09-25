# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is published and externally audited. The completed
v0.69.0 and v0.70.0 implementation work is included in the v0.71.0 Guaranteed Structural
Operations release candidate rather than receiving separate public archives. Linux target work
remains deliberately deferred.

## Next Work

Qualify the exact v0.71.0 release-content commit through the complete source gate, two deterministic
package builds, and the installed-home gate. Record the retained archive identity, then publish and
audit that exact artifact without rebuilding it. Publication is explicitly authorized for this
release.

Preserve the v0.68.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. `notrap async` is deliberately rejected because `future T` does not retain a
drive-time trap contract.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
