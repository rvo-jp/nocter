# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 3 is in progress on
`develop-v0.49.0`. Public `Stdio`, `ProcessIo`, `Child`, and pipe endpoint values now have one shared
ownership model and a qualified `spawn_blocking` path. Descriptor transfer is exact-once, untaken
endpoints close before observation, and child/endpoint destruction remains cleanup-safe.

## Next Work

Complete v0.49.0 Phase 3 with a canonical executor-safe `Command.spawn`. A blocking implementation
cannot be called from an async body, so do not alias or wrap `spawn_blocking`. Introduce one closed
nonblocking launch authority whose future owns every prepared command, report channel, child, and
endpoint across cancellation; then prove async spawn, endpoint I/O, observation, and cancellation
in a generated native image.

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
