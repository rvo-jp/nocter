# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. The v0.44.0 candidate is qualified on
`develop-v0.44.0`; public latest-release references remain at v0.43.0 until the publication commit.
Two independent complete gates and two independent optimized package builds passed, and the
retained archive and installed home have fixed recorded identities.

## Next Work

Publish v0.44.0 from the retained qualified archive without rebuilding it. Update public latest
references, create the publication commit and annotated tag, fast-forward `main`, push the exact
commits and tag, upload exactly one GitHub release asset, then download and compare that public asset
byte for byte before recording the immutable publication audit.

Preserve every published tag and asset, including v0.43.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
