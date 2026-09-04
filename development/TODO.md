# Nocter Development Handoff

## Current State

Nocter v0.33.0 is [published and externally audited](releases/v0.33.0.md). Its tag and release asset
are immutable.

The v0.34.0 Unicode-scalar implementation is complete, but its release candidate is reopened after
the final structural review found source-generation, scalar-validation, and build-cache authority
problems. The old retained archive predates the corrective commits and must not be published.
Current implementation and evidence belong to the [milestone](milestones/v0.34.0.md), its
[reviews](reviews/README.md), and the
[release-preparation record](milestones/v0.34.0-release-preparation.md).

## Next Work

Finish the reopened v0.34.0 qualification gate. Create a new release-content commit, run the full
disposable compiler verification and release qualification, replace the obsolete local candidate,
and record its exact identity. Stop before tag creation, push, upload, or public latest-release
changes unless the user separately authorizes publication.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
