# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. Annotated tag
`v0.65.0` resolves to publication commit `f470855968f8047c79880aa98d57acabac2324cf`, and the public
single asset matches the retained qualified candidate byte for byte.

## Next Work

Choose the next milestone from practical application needs. Preserve the shared-storage,
notification, bounded-coordination, service-lifecycle, application-data, routing, session, and
logging ownership boundaries instead of adding parallel authorities.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.65.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
