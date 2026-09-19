# Nocter Development Handoff

## Current State

Nocter v0.60.0 is published and externally audited. The exact v0.61.0 Reliable Development and
Distribution candidate passed the complete compiler gate and formal double-generation
qualification. Publication is authorized, and public metadata now selects v0.61.0.

## Next Work

Commit this publication metadata, create annotated tag `v0.61.0`, fast-forward `main`, and publish
the exact retained candidate as the release's single asset without rebuilding it. Then audit the
public tag, asset, latest-release endpoint, downloaded installed home, and source-identified Pages
deployment. Do not treat release checksums or manifest content digests as authenticity guarantees.

Preserve the v0.60.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No implementation, qualification, or authorization blocker remains.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
