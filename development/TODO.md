# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phase 0 has completed the Darwin TLS
provider evaluation and generalized runtime library loading. The provider decision is intentionally
open because no available choice preserves both the accepted descriptor architecture and a public
TLS 1.3 contract.

## Next Work

Choose the provider direction recorded by
[`development/design/secure-transport-design.md`](design/secure-transport-design.md), then replace
the provisional v0.43.0 phases with an exact implementation boundary before adding TLS source APIs.

Preserve every published tag and asset.

## Blockers

The v0.43.0 provider choice requires a product decision. Secure Transport preserves the current
TCP/reactor architecture but guarantees only TLS 1.2 through a deprecated API. Network.framework
provides TLS 1.3 but requires a broader network architecture replacement. An embedded provider
first requires a maintained native-object link and update boundary.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
