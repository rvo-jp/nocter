# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is qualified and its public metadata selects the
retained candidate. Phases 0 through 5 are complete. Every ordinary and compiler-generated Machine
body now follows one draft, target-independent optimization, immutable freeze, and dataflow path.
One exhaustive effect authority conservatively distinguishes pure, trapping, and observable
operations. The optimizer now removes unreachable control flow and the complete closure of dead
values, operations, stack objects, addresses, drop flags, and pack state through one dense
remapping pass. Each frozen body retains its exact transformation report.

## Next Work

Publish annotated tag `v0.68.0` and the exact retained qualified archive without rebuilding it.
Then verify the tag, single GitHub Release asset, latest-release endpoint, downloaded installed
home, push-triggered workflows, and source-identified Pages deployment in a release audit. v0.67.0
remains the externally audited release boundary until v0.68.0 publication completes.

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
