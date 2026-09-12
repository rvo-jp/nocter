# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. v0.48.0 Phases 0 through 2 are complete on
`develop-v0.48.0`: both execution surfaces now have generic buffered byte adapters over their
ordinary interfaces. `BlockingBufReader<R>` and `BlockingBufWriter<W>` are explicit synchronous
types; `BufReader<R>` and `BufWriter<W>` are canonical executor-safe types with cancellation-stable
reader scratch and terminal-before-await writer transitions. No type is tied to `File`, and no old
blocking name or close-shaped wrapper API remains.

## Next Work

Implement v0.48.0 Phase 3 as one portable state-machine qualification boundary. Exercise both
memory-backed and transport-backed adapters, malformed read counts, zero capacity and progress,
UTF-8 scalars split across refills, CRLF and unterminated lines, nested flush, explicit failure,
cancellation, and exact-once cleanup. The tests must prove that scratch initialization and ambiguous
output prefixes never become retryable user data.

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
