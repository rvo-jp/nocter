# Nocter Development Handoff

## Current State

Nocter v0.45.0 is published and externally audited. The v0.46.0 release candidate is qualified from
release-content commit `ad30785c52479bd123c419887e081381051831c0`, and its retained archive is
`dist/nocter-v0.46.0-arm64-darwin.tar.gz`. Public latest-release references remain at v0.45.0 until
an explicitly authorized publication commit. Asynchronous datagrams cross closed target
operations, shared blocking/async policy, the public standard surface, native IPv4/IPv6 execution,
cancellation and timeout behavior, a complete public example, and ordinary checked LSP queries.

## Next Work

On explicit publication authorization, reuse the retained qualified archive without rebuilding it,
update public latest-release references, create the annotated `v0.46.0` tag, upload the one archive,
and audit the public download byte for byte. The implementation and qualification contracts are in
the [v0.46.0 milestone](history/milestones/v0.46.0.md),
[Phase 5 final review](history/reviews/v0.46.0-phase-5.md), and
[release-preparation record](history/milestones/v0.46.0-release-preparation.md). Do not tag, push,
upload, or change public latest-release references without explicit authorization.

Preserve every published tag and asset, including v0.45.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
