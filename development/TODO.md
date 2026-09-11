# Nocter Development Handoff

## Current State

Publication of the qualified v0.46.0 candidate is authorized and in progress. The retained archive
was built from release-content commit `ad30785c52479bd123c419887e081381051831c0`; its identity must
remain unchanged through the public audit. Asynchronous datagrams cross closed target
operations, shared blocking/async policy, the public standard surface, native IPv4/IPv6 execution,
cancellation and timeout behavior, a complete public example, and ordinary checked LSP queries.

## Next Work

Integrate the public latest-release surfaces into `main`, create and push one annotated `v0.46.0`
tag, upload the retained archive as the release's only asset, and verify the public tag,
latest-release endpoint, asset bytes, extracted installation, and remote `main`. Record that
evidence and stop.

Preserve every published tag and asset, including v0.45.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
