# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. v0.48.0 Phase 0 is complete on
`develop-v0.48.0`: the generic buffered-I/O ownership, progress, failure, cancellation, and naming
contracts are fixed without adding compiler-recognized library names. Unqualified `BufReader<R>`
and `BufWriter<W>` will follow the canonical asynchronous `Reader` and `Writer` contracts; explicit
`BlockingBufReader<R>` and `BlockingBufWriter<W>` will replace the current file-specific blocking
types without compatibility aliases.

## Next Work

Implement v0.48.0 Phase 1 as one closed migration: make the blocking buffers generic over their
public contracts, rename them explicitly, migrate every standard source, example, fixture, and
tooling expectation, and remove the file-specific representation. Preserve the exact public
blocking line and flush behavior while making ownership of the underlying stream generic.

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
