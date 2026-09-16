# Nocter Development Handoff

## Current State

Nocter v0.54.0 is published and externally audited. v0.55.0 implementation is complete and ready
for a separate release-preparation pass. It adds explicit owning callable erasure and uses that
single runtime-dispatch model for practical HTTP routing and a complete application. The adopted
boundary lives in
[`development/design/erased-callable-design.md`](design/erased-callable-design.md), and phase state
lives in [`development/history/milestones/v0.55.0.md`](history/milestones/v0.55.0.md).

## Next Work

Prepare and publish v0.55.0 without changing the qualified language, compiler, standard-library,
or application behavior. The final implementation review is recorded in
[`development/history/reviews/v0.55.0-phase-5.md`](history/reviews/v0.55.0-phase-5.md).

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
