# Nocter Development Handoff

## Current State

Nocter v0.42.0 is published and externally audited. The public download is byte-identical to the
qualified archive, the annotated tag resolves to the publication commit, and GitHub reports it as
the latest release.

## Next Work

Define the next release boundary before changing source. Preserve the immutable v0.41.0 and
v0.42.0 tags and assets; any correction requires a new version and complete qualification.

Preserve every published tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
