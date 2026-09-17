# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 is locally qualified from exact
release-content commit `180bac07b66fd031eade3a1fd40fd0fcdc219245`: two independent optimized
builds produced one deterministic archive, and the complete compiler, installed-home, native,
example, and LSP gates pass. The completed implementation scope lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Publish v0.57.0 from the retained qualified archive without rebuilding it, then audit the public
tag, release asset, latest-release endpoint, downloaded installed home, remote `main`, and Pages
deployment before marking the release complete.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
