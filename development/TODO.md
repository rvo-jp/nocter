# Nocter Development Handoff

## Current State

Nocter v0.64.0 Application Encoding and Identity is published and externally audited. The exact
v0.65.0 Practical Stateful Services candidate from release-content commit
`8744a1ecf8ea13a9369b61a366c6c343afddd355` passed the complete compiler gate and deterministic
artifact qualification. Public metadata now selects v0.65.0; publication and external audit remain.

## Next Work

Publish annotated tag `v0.65.0` and the exact retained qualified archive with SHA-256
`24fa1607f6faf1ea2e0efee21885dde20d7a4e5648fb86d4a7f8d67e2dce65de`, without rebuilding it. Then
audit the public tag, single release asset, downloaded installation, Actions runs, latest-release
endpoint, and source-identified Pages deployment.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.65.0 publication.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
