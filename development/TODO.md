# Nocter Development Handoff

## Current State

Nocter v0.61.0 is published and externally audited. v0.62.0 Structured Command-Line Applications
is implementation-complete and qualified. Release-content commit
`3f6f1eb3f68c96ff36453f9d0d42f349e219731d` passed the complete gate and deterministic artifact
qualification; the exact retained archive is ready for publication.

## Next Work

Wait for explicit publication authorization. Publication must reuse the retained
`dist/nocter-v0.62.0-arm64-darwin.tar.gz` archive with SHA-256
`39d07ee1a89874c77f096eab9c1c1f5be8f79e0ef737159db86ddfe2bbb296ed` and must not rebuild it.

Preserve the v0.61.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.62.0 release preparation.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
