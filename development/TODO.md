# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. v0.40.0 is active as one synchronous Internet
client foundation spanning trusted target services, URLs, system name resolution, HTTP/1.1 framing,
and an owned synchronous client lifecycle.

## Next Work

Complete Phase 0 by implementing the source-independent trusted target-service catalog and the
minimal Darwin C-call/Mach-O import path required by system name resolution. Keep this toolchain-
private: do not introduce public FFI or URL, resolver, or HTTP meaning into semantic IR. Preserve
the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
