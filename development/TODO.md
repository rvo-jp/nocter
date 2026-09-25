# Nocter Development Handoff

## Current State

Nocter v0.71.0 Guaranteed Structural Operations is published and externally audited. The completed
v0.69.0 and v0.70.0 implementation milestones are included in this release rather than receiving
separate public archives. Linux target work remains deliberately deferred.

## Next Work

Begin v0.72.0 with a flow-proof model for safe operations. The checker must derive local range and
arithmetic facts from control flow, freeze admitted proof in the checked program, and let later
lowering consume only that proof. Do not add unchecked indexing, source-text rediscovery, or a
second backend-owned proof system.

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
