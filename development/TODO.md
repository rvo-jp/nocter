# Nocter Development Handoff

## Current State

Nocter v0.40.0 is published and externally audited. The exact v0.41.0 release candidate is
qualified and retained locally from release-content commit
`d63f749f83847bf445112b49e94173b315233ee2`. Public latest-release references remain at v0.40.0.

## Next Work

Wait for explicit publication authorization. Publication must reuse the retained qualified archive
without rebuilding it, update public latest-release surfaces in a separate commit, create and push
one annotated `v0.41.0` tag, upload exactly one asset, and verify the public asset byte for byte.

Preserve the immutable v0.39.0 and v0.40.0 tags and assets.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
