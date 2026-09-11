# Nocter Development Handoff

## Current State

Nocter v0.46.0 is published and externally audited. v0.47.0 Phase 0 is complete on
`develop-v0.47.0`. The new milestone makes the unqualified `Reader` and `Writer` names canonical
asynchronous byte-stream contracts, gives synchronous contracts explicit `BlockingReader` and
`BlockingWriter` names, and removes duplicated concrete collection logic through generic defaults.

## Next Work

Migrate the synchronous interfaces and all consumers to `BlockingReader` and `BlockingWriter`
without aliases before introducing their canonical asynchronous replacements. The
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
