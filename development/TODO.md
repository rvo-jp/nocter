# Nocter Development Handoff

## Current State

Nocter v0.66.0 Durable Local Application State is published and externally audited. Its exact
release-content commit passed the complete compiler gate and deterministic artifact qualification;
the public tag, single asset, downloaded installed home, latest-release endpoint, Actions runs,
and source-identified Pages deployment all agree on the published identity.

## Next Work

Define the next application milestone from current user priorities before changing the published
contract. Preserve the v0.66.0 release boundary and begin new behavior under a new versioned
milestone rather than extending the completed record.

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
