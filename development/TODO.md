# Nocter Development Handoff

## Current State

Nocter v0.33.0 is [published and externally audited](releases/v0.33.0.md). Its tag and release asset
are immutable.

The v0.34.0 Unicode-scalar implementation and corrective review are complete. Release-content
commit `b876301201105e061000e2d48c8d18246d11814d` has a locally qualified, reproducible candidate.
Current implementation and evidence belong to the [milestone](milestones/v0.34.0.md), its
[reviews](reviews/README.md), and the
[release-preparation record](milestones/v0.34.0-release-preparation.md).

## Next Work

Publish the already qualified v0.34.0 archive described by the
[release-preparation record](milestones/v0.34.0-release-preparation.md). Publication is authorized:
advance public latest links in a separate commit, tag that publication state once, upload exactly
the retained archive, and audit its public bytes. Do not rebuild or replace the candidate.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
