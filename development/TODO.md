# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. v0.48.0 Phases 0 through 3 are complete on
`develop-v0.48.0`: both execution surfaces now have qualified generic buffered byte adapters over their
ordinary interfaces. `BlockingBufReader<R>` and `BlockingBufWriter<W>` are explicit synchronous
types; `BufReader<R>` and `BufWriter<W>` are canonical executor-safe types with cancellation-stable
reader scratch and terminal-before-await writer transitions. No type is tied to `File`, and no old
blocking name or close-shaped wrapper API remains. Native qualification covers one-byte TCP
refills, memory-backed suspension, malformed progress, failure, cancellation, and exact-once drop.

## Next Work

Implement v0.48.0 Phase 4 as one application and tooling integration boundary. Add a complete
asynchronous line-oriented public application over TCP or TLS, reuse the async buffer contract over
a second concrete stream, and qualify hover, completion, signature help, navigation, semantic
tokens, and inlay hints through ordinary generic/interface evidence. Update only the existing I/O
behavior authority; do not duplicate declarations in prose.

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
