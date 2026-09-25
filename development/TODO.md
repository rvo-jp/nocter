# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is published and externally audited. v0.69.0
Canonical Declaration Contracts and v0.70.0 Trap-Free Callable Contracts are complete but
unpublished. v0.71.0 Guaranteed Structural Operations is implementation-complete and unpublished.
Linux target work remains deliberately deferred.

## Next Work

Prepare the v0.71.0 release identity and qualify the deterministic installed artifact when release
preparation is authorized. Re-run the complete source gate from the final release-content commit,
then retain and audit the exact qualified archive. Do not tag, upload, or publish without explicit
authorization.

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
