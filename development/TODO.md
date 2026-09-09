# Nocter Development Handoff

## Current State

Publication of the qualified v0.42.0 candidate is authorized and in progress. The retained archive
was built from release-content commit `a2b6156bc1db715804218cbab3a0f68a3a6aad72`; its identity must
remain unchanged through the public audit.

## Next Work

Commit the public latest-release surfaces, integrate them into `main`, create and push one annotated
`v0.42.0` tag, upload the retained archive as the release's only asset, and verify the public tag,
latest-release endpoint, asset bytes, extracted installation, and remote `main`. Record that
evidence and stop.

Preserve every published tag and asset, including v0.41.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
