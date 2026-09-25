# Nocter Development Handoff

## Current State

Nocter v0.71.0 Guaranteed Structural Operations is published and externally audited. v0.72.0
Flow-Proven Safe Operations is active. Phase 0 freezes direct fixed-array bounds dispositions in
checking and carries them through MIR and Machine without backend re-proof; its implementation,
qualification, and authority review are complete. Linux target work remains deliberately deferred.

## Next Work

Begin v0.72.0 Phase 1 by introducing the checking-owned control-flow fact domain for branch and join
refinement. Extend safety admission only through that shared domain before applying it to checked
arithmetic and standard-library guarantees. Do not add unchecked indexing, source-text rediscovery,
or a second backend-owned proof system.

Preserve the v0.71.0 release-content commit, publication tag, retained asset, release notes,
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
