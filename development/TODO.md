# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. The exact v0.40.0 release candidate is qualified
and retained locally. Public latest-release references remain at v0.39.0 until publication is
separately authorized. v0.41.0 planning is isolated on `develop-v0.41.0`; no release identity has
been changed.

## Next Work

Continue v0.41.0 Phase 3 from the
[Asynchronous Computation Boundary](design/asynchronous-computation-design.md). The scheduler,
Darwin reactor adapter, opaque single-threaded executor contract, ARM64 async heap-frame placement,
and constructor/resume/cancel/consume entries are complete. Whole-program lowering now emits the
complete lifecycle, and native conformance covers lazy construction followed by cancellation.

The compiler-generated process adapter now accepts the six process-result contracts beneath one
`async` layer, owns the lazy root computation, and drives it through one readiness-backed Darwin
wait boundary. The first compiler-owned descriptor-readiness computation now constructs the shared
opaque ABI directly, publishes a frame-owned interest, and completes after resumption. Native
conformance holds a generated process on an unreadable pipe before releasing it from the parent, so
the complete pending-to-host-wait-to-resume path is exercised without busy polling.

The monotonic-deadline producer now shares one resume/cancel/consume lifecycle with descriptor
readiness while retaining its own constructor. Native conformance proves that a generated process
cannot complete before the requested absolute deadline. Next, expose the qualified timer through a
small public asynchronous time contract, then add concurrent Darwin loopback coverage before
exposing asynchronous networking. Keep the qualified v0.40.0 archive unchanged.

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
