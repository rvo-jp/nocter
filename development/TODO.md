# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Phases 0 through 7 are complete. Its completion definition and
phased authority replacement live in
[`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Prepare the v0.50.0 release candidate without changing the published v0.49.0 entry points. Advance
the single packaging version authority and standard-package declaration together, write the public
release record, run the complete compiler gate in a disposable target, and qualify two identical
installed archives. Do not tag, push, upload, or rewrite latest-version documentation until
publication is explicitly requested.

Phase 7 added installed execution of `async-file-report` and semantic editor checks against its
real source. Preserve exact standard-module dependency review, canonical formatting for every
runnable example, bounded application memory, and failure cleanup that leaves no temporary or
failed final output.

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
