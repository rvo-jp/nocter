# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is implementation-complete and has a deterministically
qualified `arm64-darwin` release candidate retained under `dist/`. The release-content commit is
`8744a1ecf8ea13a9369b61a366c6c343afddd355`. The previous v0.64.0 Application Encoding and Identity
release remains the latest published release and is externally audited.

## Next Work

Wait for explicit publication authorization. Publication must reuse
`dist/nocter-v0.65.0-arm64-darwin.tar.gz` with SHA-256
`24fa1607f6faf1ea2e0efee21885dde20d7a4e5648fb86d4a7f8d67e2dce65de`; do not rebuild it. After
publication, verify the annotated tag, GitHub Release asset, latest-release endpoint, downloaded
installed home, and source-identified Pages deployment in a release audit.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

Publication is intentionally blocked on explicit user authorization. No qualification blocker
remains.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
