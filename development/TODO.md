# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. The annotated tag, single GitHub Release
asset, latest-release endpoint, downloaded installed home, and source-identified Pages deployment
all match the qualified candidate and publication commit.

## Next Work

Plan the next milestone from concrete user-facing or standard-library gaps. Preserve the v0.62.0
tag, release asset, public notes, specification snapshot, and publication audit without
replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.62.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
