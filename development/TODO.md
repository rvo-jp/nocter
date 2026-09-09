# Nocter Development Handoff

## Current State

Nocter v0.42.0 is qualified on the development branch with no open practical finding. Its retained
archive was built reproducibly from release-content commit
`a2b6156bc1db715804218cbab3a0f68a3a6aad72`; public latest-release references remain at v0.41.0.

## Next Work

Await explicit publication authorization. Publication must reuse the retained qualified archive,
must not rebuild it, and must keep the annotated tag, GitHub release asset, public latest-release
references, and external byte-for-byte audit on one recorded identity.

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
