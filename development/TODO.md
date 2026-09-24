# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is published and externally audited. v0.69.0
Canonical Declaration Contracts and v0.70.0 Trap-Free Callable Contracts are complete but
unpublished. Linux target work remains deliberately deferred.

## Next Work

Begin v0.70.0 release preparation. Carry the completed v0.69.0 declaration-contract work and
v0.70.0 trap-free contract into one release candidate, update versioned public surfaces, build the
archive from clean committed source, and qualify the exact archive before publication. Do not add a
`realtime` profile or asynchronous future-drive guarantee during release preparation.

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
