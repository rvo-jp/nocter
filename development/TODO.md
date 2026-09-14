# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Phases 0 and 1 are complete; Phase 2 is active. Its completion definition and
phased authority replacement live in
[`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Continue v0.50.0 Phase 2 with links, canonicalization, identity-safe bounded copy, recursive
removal, and traversal. The completed ranges have
completed canonical asynchronous and explicit blocking twins for single-entry removal, rename,
single-directory creation, empty-directory removal, metadata, and existence queries over
owned-path jobs. Directory acquisition and raw record batches now use the same bounded service,
completion-time caller-buffer publication, and pre-reserved retirement authority. The asynchronous
and blocking streams share one record decoder. Metadata target storage is decoded once by the
worker and only portable facts cross the completion ABI. Recursive directory construction has
canonical asynchronous and explicit blocking forms over one pure prefix scanner. Preserve one
path-validation authority, one portable error policy, bounded recursive work, and typed target
facts. Do not implement an asynchronous surface by calling a public blocking wrapper or by
retaining caller storage in a worker.

Phase 1 closed the canonical file cutover. `File` is executor-safe; `BlockingFile` is its explicit
synchronous twin. Separate semantic primitive roles prevent standard source from constructing
access-mode or seek-origin tags. Opaque runtime storage retains its physical category through MIR
and Machine destruction. Native qualification covers construction, every operation family,
explicit close, implicit drop, and process shutdown; generated lifecycle tests cover cancellation
and abandonment without scheduler races.

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
