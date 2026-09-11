# Nocter Development Handoff

## Current State

Nocter v0.46.0 is published and externally audited. The public asset is byte-identical to the
retained qualified archive built from release-content commit
`ad30785c52479bd123c419887e081381051831c0`. Asynchronous datagrams cross closed target operations,
shared blocking/async policy, the public standard surface, native IPv4/IPv6 execution,
cancellation and timeout behavior, a complete public example, and ordinary checked LSP queries.

## Next Work

Plan the next milestone only when requested. Preserve the separation between closed target
operations, standard-source retry and timeout policy, executor readiness, positive synchronous
`blocking`, and result provenance; do not infer one fact from another.

Preserve every published tag and asset, including v0.46.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
