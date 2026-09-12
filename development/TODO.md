# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 2 is complete on
`develop-v0.49.0`. Every synchronous launch now acquires one private child owner immediately after
fork. Exact terminal observation disarms it; every remaining owner synchronously terminates its
child and transfers only reaping to a compiler-owned Darwin worker without blocking the caller.

## Next Work

Implement v0.49.0 Phase 3 as one public process-value change: add `Stdio`, `ProcessIo`, `Child`,
`ChildStdin`, `ChildStdout`, and `ChildStderr`; expose async and explicit blocking spawn and
observation; and make endpoint transfer consume exactly one owner while all untaken endpoints
remain cleanup-safe.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
