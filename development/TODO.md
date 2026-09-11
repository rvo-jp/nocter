# Nocter Development Handoff

## Current State

Nocter v0.45.0 is published and externally audited. v0.46.0 implementation and qualification are
complete on `develop-v0.46.0`. Asynchronous datagrams cross closed target operations, shared
blocking/async policy, the public standard surface, native IPv4/IPv6 execution, cancellation and
timeout behavior, a complete public example, and ordinary checked LSP queries without weakening
the universal `future T` drive invariant.

## Next Work

Begin v0.46.0 release preparation from the completed
[milestone](history/milestones/v0.46.0.md) and
[Phase 5 final review](history/reviews/v0.46.0-phase-5.md). Change release identity only as one
coherent preparation step, build and inspect a fresh distribution, verify its adjacent `.nocter`
home and public async UDP example, then prepare release notes and the publication commit. Do not
tag or publish without explicit authorization.

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
