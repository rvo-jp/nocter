# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. The v0.50.0 Local Data and Asynchronous
Streaming candidate is complete and qualified. Its completion definition and phased authority
replacement live in
[`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Await explicit publication authorization. The retained qualified archive belongs to release-
content commit `b3bfaff4e36882c3616d2aa84ccddedf694ea62d`; publication must reuse it without rebuilding.
Only the publication transaction may create the annotated tag, push commits and the tag, upload
the single asset, update public latest-version entry points, and deploy the source-identified
documentation artifact.

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
