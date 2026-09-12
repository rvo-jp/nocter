# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. The public asset is byte-identical to the
retained qualified archive built from release-content commit
`e191c63a881899e9e1df833184111d0ef4116026`. The release makes
unqualified `Reader` and `Writer` canonical asynchronous byte-stream contracts, gives synchronous
contracts explicit `BlockingReader` and `BlockingWriter` names, removes duplicated concrete
collection logic through generic defaults, and closes compiler convention and source-snapshot
authorities.

## Next Work

Plan the next milestone only when requested. Preserve canonical asynchronous `Reader` and `Writer`
contracts, explicit blocking twins, transport-owned readiness and timeout policy, frozen semantic
dispatch, and source-snapshot identity; do not infer one fact from another.

Preserve every published tag and asset, including v0.47.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
