# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. The v0.50.0 Local Data and Asynchronous
Streaming candidate is complete and qualified, publication is authorized, and public metadata now
selects v0.50.0. Its completion definition and phased authority replacement live in
[`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Commit this publication metadata, create annotated tag `v0.50.0`, fast-forward `main`, upload the
retained qualified archive as the release's single asset, and verify the public tag, release asset,
latest-release endpoint, downloaded archive, installed identity, and source-identified Pages
deployment. Do not rebuild or replace the qualified archive.

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
