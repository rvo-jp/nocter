# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is published and externally audited. The post-release
file-retirement correction is on `main` and will be included in the next release without changing
the retained v0.67.0 tag or artifact.

v0.68.0 Production Native Performance is active. Phases 0 through 3 are complete. Every ordinary and
compiler-generated Machine body now follows one draft, target-independent optimization, immutable
freeze, and dataflow path. One exhaustive effect authority conservatively distinguishes pure,
trapping, and observable operations. The optimizer now removes unreachable control flow and the
complete closure of dead values, operations, stack objects, addresses, drop flags, and pack state
through one dense remapping pass. Each frozen body retains its exact transformation report.

## Next Work

Implement v0.68.0 Phase 4 against the optimized immutable Machine product. Exercise synchronous,
fallible, allocation-heavy, asynchronous, cancellation, collection, parsing, compression,
filesystem, process, networking, and HTTP workloads; record generated code size and runtime with
complete environment/sample identity; and attribute remaining dominant costs before making any
target-specific change. Phase 3 completed same-block storage forwarding, unobserved stack removal,
drop-flag coalescing, physical storage aliases, lane-complete register aggregate construction, and
Machine-owned constant-index proofs. Fixed indexes now freeze as required, proven in bounds, or
proven to trap; ARM64 consumes that disposition without repeating range analysis. Stack-rooted
constant projections become non-trapping only after their complete byte extent and alignment are
proven inside the stack object. Calls, suspension, ownership transfer, unknown aliasing,
dereferences, views, dynamic offsets, unresolved bounds, and arithmetic traps remain barriers.

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
