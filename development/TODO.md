# Nocter Development Handoff

## Current State

Nocter v0.46.0 is published and externally audited. v0.47.0 Phases 0–2 are complete on
`develop-v0.47.0`. The new milestone makes the unqualified `Reader` and `Writer` names canonical
asynchronous byte-stream contracts, gives synchronous contracts explicit `BlockingReader` and
`BlockingWriter` names, and removes duplicated concrete collection logic through generic defaults.

## Next Work

Make TCP, TLS, and HTTP response bodies implement the canonical async interfaces, remove the
response-specific ordinary collector, and retain timeout policy only on concrete methods. The
[v0.47.0 milestone](history/milestones/v0.47.0.md) owns phase gates; the
[asynchronous byte-I/O design](design/asynchronous-byte-io-design.md) owns the cross-module
contract and information flow.

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
