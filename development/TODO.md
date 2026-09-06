# Nocter Development Handoff

## Current State

The v0.38.0 release candidate is qualified. The retained archive was built from release-content
commit `ae442a6ff01be5d35b73d69729e3fb25e30ddd6e`; the public latest release remains v0.37.0 until
publication is separately authorized.

## Next Work

Wait for explicit publication authorization. Publication must reuse the retained qualified archive
without rebuilding it, then update the public latest-release surfaces, create and push the
annotated tag, upload the one archive, and verify the public download byte for byte.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
