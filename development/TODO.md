# Nocter Development Handoff

## Current State

The v0.36.0 implementation and final design review are complete. The release identity is frozen at
`0.36.0`; the public latest release remains v0.35.0 until publication is separately authorized.

## Next Work

Qualify the exact clean v0.36.0 release-content commit through two independent compiler gates,
explicit public-HTTPS acquisition, deterministic packaging, and installed-home validation. Record
the retained archive identity and stop before tagging, pushing, uploading, or changing public
latest-release links.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
