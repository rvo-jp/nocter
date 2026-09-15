# Nocter Development Handoff

## Current State

Nocter v0.53.0 development is active. Phases 0 through 2 are complete: JSON compact generation uses
one effect-neutral pull encoder, and `TaskGroup<T>` now provides runtime-sized structured ownership
without detached work or a second executor. The active scope and completion gates live in
[`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Implement Phase 3 as one HTTP server lifecycle over existing asynchronous network streams,
`TaskGroup`, timeouts, and incremental codecs. Define bounded request framing and connection close
ownership before adding convenience APIs. Do not introduce a compiler-recognized service,
detached connection task, hidden unbounded buffer, or second transport lifecycle.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
