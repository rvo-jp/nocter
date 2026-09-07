# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. The exact v0.39.0 release-content commit passed
the complete release matrix, and its qualified archive is retained under `dist/`. Public
latest-release references remain at v0.38.0 until publication is separately authorized.

## Next Work

Await explicit publication authorization. Publication must reuse the retained qualified v0.39.0
archive without rebuilding it, update public latest-release references in a separate commit, create
one annotated tag, upload exactly one asset, and audit the public download byte for byte. Preserve
the immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
