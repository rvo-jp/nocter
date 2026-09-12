# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. v0.48.0 Phases 0 and 1 are complete on
`develop-v0.48.0`: the buffered-I/O ownership, progress, failure, cancellation, and naming contracts
are fixed, and the blocking surface is now generic `BlockingBufReader<R>` and
`BlockingBufWriter<W>` over the ordinary blocking interfaces. The old file-specific representation,
close-shaped wrapper API, and unqualified blocking names have no compatibility path.

## Next Work

Implement v0.48.0 Phase 2 as one asynchronous state-machine boundary. Add canonical generic
`BufReader<R>` and `BufWriter<W>` over `Reader` and `Writer`, including line reuse, complete writes,
flush, consuming finish, cancellation-safe terminal transitions, and native qualification for both
immediate and suspended progress. Share only pure byte, capacity, validation, and error policy with
the blocking surface; do not merge their control loops or add compiler-recognized library names.

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
