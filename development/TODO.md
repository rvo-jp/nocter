# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. v0.40.0 is active as one synchronous Internet
client foundation spanning trusted target services, URLs, system name resolution, HTTP/1.1 framing,
and an owned synchronous client lifecycle.

## Next Work

Begin Phase 1 with the immutable `std/url` value boundary. Define URL components and parsing before
transport integration; parsing, canonical formatting, hashing, equality, relative resolution, and
HTTP request-target projection must consume one retained representation. Keep DNS, sockets, and
HTTP policy outside the URL module. Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
