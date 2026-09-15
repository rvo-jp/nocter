# Nocter Development Handoff

## Current State

Nocter v0.52.0 is published and externally audited. The v0.53.0 Structured Services and Streaming
Codecs candidate is complete and qualified, publication is authorized, and public metadata now
selects v0.53.0. The retained archive and measured identities are recorded in the release-preparation
record. The completed implementation scope lives in
[`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Commit this publication metadata, create annotated tag `v0.53.0`, fast-forward `main`, upload the
retained qualified archive as the release's single asset, and verify the public tag, release asset,
latest-release endpoint, downloaded archive, installed identity, and source-identified Pages
deployment. Do not rebuild or replace the qualified archive.

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
