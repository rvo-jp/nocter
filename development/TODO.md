# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. The corrected v0.60.0 Streaming Compression
and Safe Archives candidate is qualified, publication is authorized, and public metadata now
selects v0.60.0. The earlier candidate and its digest are superseded and must not be published.
The retained replacement archive and measured identities are recorded in the release-preparation
record.

## Next Work

Commit this publication metadata, create annotated tag `v0.60.0`, fast-forward `main`, and publish
the retained replacement archive as the release's single asset. Then audit the public tag, asset,
latest-release endpoint, downloaded installed home, and source-identified Pages deployment. Do
not rebuild or replace the qualified archive.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No implementation, qualification, or authorization blocker remains.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
