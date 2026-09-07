# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. v0.40.0 Phases 0-2 are complete. The active
release remains one synchronous Internet client foundation spanning trusted target services, URLs,
system name resolution, HTTP/1.1 framing, and an owned synchronous client lifecycle.

## Next Work

Begin Phase 3 with validated HTTP/1.1 message values and one transport-independent framing codec.
Keep method, status, header, body-framing, syntax-limit, and buffer-limit decisions independent of
DNS and socket ownership. Select message framing exactly once, reject ambiguous or unbounded input
before allocation, and cover partial input, informational responses, trailers, premature EOF,
overflow, conflicting lengths, unsupported transfer codings, and smuggling-prone combinations.
Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
