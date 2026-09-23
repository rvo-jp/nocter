# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is qualified and ready for publication. All eight
phases are complete: the
milestone contract is fixed; `Store` ownership is enforced by one target-backed non-waiting lock;
one insertion-ordered `StoreState` owns lookup and traversal; journal version `2` publishes bounded
mutation batches with deterministic checkpoint fallback while explicitly normalizing the published
v0.66.0 format; and `std/config` owns source-neutral schemas, failure-atomic overlays, validation,
provenance, immutable results, and secret-safe presentation. Scalar JSON objects, explicitly
selected process environment variables, structured command-line results, and authored defaults now
enter that same source vocabulary without adapter-owned precedence or validation. One process-owned
lifecycle source now distinguishes consumable reload from sticky termination, and
`PublishedConfiguration` can expose only a fully validated immutable value while retaining the
previous value after candidate failure. The complete HTTP service now applies those contracts to
bounded concurrent admission, incremental durable events, installed-home reload, rejected-candidate
retention, exclusive store ownership, redacted output, graceful shutdown, and restart recovery.
v0.66.0 remains the published and externally audited release boundary until v0.67.0 publication.

## Next Work

Publish `dist/nocter-v0.67.0-arm64-darwin.tar.gz` with SHA-256
`8d370918223ace0b1583f873bb47b826888ee5c531c1dcbd9d54eea748dc2f9a` without rebuilding it. Then
verify the annotated tag, single GitHub Release asset, latest-release endpoint, downloaded
installed home, push-triggered workflows, and source-identified Pages deployment in a release
audit.

Preserve the v0.66.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.66.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
