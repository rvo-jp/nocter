# Nocter Development Handoff

## Current State

Nocter v0.71.0 Guaranteed Structural Operations is published and externally audited. v0.72.0
Flow-Proven Safe Operations is active. Phase 0 freezes direct fixed-array bounds dispositions in
checking and carries them through MIR and Machine without backend re-proof; its implementation,
qualification, and authority review are complete. Phase 1 adds the checking-owned flow fact domain,
branch refinement, conservative joins, and immutable-value admission. Phase 2 extends that domain
to compiler-selected view lengths and loop exits without spelling-based inference. Linux target
work remains deliberately deferred.

## Next Work

Continue v0.72.0 Phase 3 by closing runtime integer-arithmetic checks and freezing their
checking-owned dispositions through MIR, Machine, and ARM64. Then apply only proven guarantees to
standard APIs. Do not add operation-specific proof tables, unchecked arithmetic, source-text
rediscovery, or a backend-owned proof system.

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
