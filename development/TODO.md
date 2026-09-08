# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. The exact v0.40.0 release candidate is qualified
and retained locally. Public latest-release references remain at v0.39.0 until publication is
separately authorized. v0.41.0 planning is isolated on `develop-v0.41.0`; no release identity has
been changed.

## Next Work

Implement v0.41.0 Phase 2 from the
[Asynchronous Computation Boundary](design/asynchronous-computation-design.md). Lower the completed
checked async product into explicit target-independent state machines with closed resume,
suspension, completion, failure, cancellation, and destruction transitions. Frame layout must
consume checked liveness and ownership facts; it must not repeat semantic selection or inspect
source syntax. Do not add an executor, reactor, or public async standard-library API until MIR
validation can prove the state and cleanup invariants. Keep the qualified v0.40.0 archive
unchanged.

Publish v0.40.0 only when explicitly requested. Publication must occur from `main`, reuse the
retained qualified archive without rebuilding it, update public latest-release references, create
one annotated tag, push the exact publication commit and tag, upload exactly one asset, and verify
the public download byte for byte. Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
