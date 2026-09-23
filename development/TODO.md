# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is published and externally audited. The post-release
file-retirement correction is on `main` and will be included in the next release without changing
the retained v0.67.0 tag or artifact.

v0.68.0 Production Native Performance is active. Phases 0 and 1 are complete. Every ordinary and
compiler-generated Machine body now follows one draft, target-independent optimization, immutable
freeze, and dataflow path. One exhaustive effect authority conservatively distinguishes pure,
trapping, and observable operations, and each frozen body retains its exact transformation report.

## Next Work

Implement v0.68.0 Phase 2 through one dense remapping authority. Remove unreachable blocks and dead
pure values together with their unused stack, address, flag, and pack identities; do not expose
sparse tables or let ARM64 reinterpret reachability. Preserve operations classified as trapping or
observable even when their results are unused.

Preserve the v0.67.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker is known.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
