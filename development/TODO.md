# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. The v0.39.0 implementation is closed and its
release identity is frozen at `0.39.0`. Public latest-release references remain at v0.38.0 until
publication is separately authorized.

## Next Work

Qualify the exact clean v0.39.0 release-content commit through two independent compiler gates, the
explicit public-HTTPS acquisition test, deterministic packaging, and complete installed-home
validation. Record the retained archive identity and stop before tagging, pushing, uploading, or
changing public latest-release links. Preserve the immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
