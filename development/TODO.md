# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 and 1 are complete: the milestone
contract is fixed, and `Store` ownership is now enforced by one target-backed non-waiting lock held
on a stable sibling for the complete owner lifetime. v0.66.0 remains the published and externally
audited release boundary.

## Next Work

Implement Phase 2 as one state-representation slice: retain deterministic ordered entries as the
sole state authority, add one transient seeded hash index for lookup and mutation, maintain the
index without exposing hash placement to persistence, and expose a lending entry iterator. Recovery
must build the index once; ordinary operations must not reconstruct it or fall back to linear scan.

Preserve the v0.66.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.66.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
