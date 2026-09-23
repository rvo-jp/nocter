# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is published and externally audited. The post-release
file-retirement correction is on `main` and will be included in the next release without changing
the retained v0.67.0 tag or artifact.

v0.68.0 Production Native Performance is active. Phases 0 through 2 are complete. Every ordinary and
compiler-generated Machine body now follows one draft, target-independent optimization, immutable
freeze, and dataflow path. One exhaustive effect authority conservatively distinguishes pure,
trapping, and observable operations. The optimizer now removes unreachable control flow and the
complete closure of dead values, operations, stack objects, addresses, drop flags, and pack state
through one dense remapping pass. Each frozen body retains its exact transformation report.

## Next Work

Implement v0.68.0 Phase 3 at the same Machine draft boundary. Forward values and local copies only
across proven alias-safe regions, remove redundant loads, stores, address materialization, borrow
weakening, aggregate staging, and drop-flag transitions, and keep calls, suspension, ownership
transfer, and unknown aliasing as explicit barriers. Safety checks may disappear only with a
Machine-owned proof that their failure condition is impossible. Exact same-block whole-stack load
forwarding and complete unobserved local-storage removal are complete; next handle
representation-preserving aliases and proven checks without weakening call, suspension, ownership,
or alias barriers. Redundant same-block drop-flag transitions are complete and stop at callable
boundaries. Exact direct whole-stack address operations now carry a non-trapping proof; general
checked address evaluation remains conservative.

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
