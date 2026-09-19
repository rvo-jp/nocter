# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. v0.60.0 qualification is reopened after
release-candidate review found and corrected complete decoded-byte accounting and an ARM64
large-copy address-lifetime defect. The earlier candidate and its digest are superseded and must
not be published. The completed implementation scope is recorded in
[`v0.60.0: Streaming Compression and Safe Archives`](history/milestones/v0.60.0.md).

## Next Work

Complete the full compiler and release qualification from a clean replacement release-content
commit. Replace `dist/nocter-v0.60.0-arm64-darwin.tar.gz`, record its exact identity, and do not
publish until the replacement compiler, application, editor, installation, immutability, and
tamper gates pass.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

The replacement v0.60.0 candidate has not yet been qualified.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
