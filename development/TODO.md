# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is active. Its first phase generalizes checking-owned path facts from
independent numeric ranges and one special length relation to normalized order between stable
values and semantic view lengths. Linux target work remains deliberately deferred.

## Next Work

Complete the [v0.73.0 milestone](history/milestones/v0.73.0.md) in order: relational facts,
place-sensitive mutation, loop invariants, borrowed range construction, standard traversal
adoption, and qualification. Do not reopen v0.72.0; preserve its checked safety dispositions,
qualified artifact, public tag, and external audit as one immutable boundary.

Preserve the v0.72.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. `notrap async` is deliberately rejected because `future T` does not retain a
drive-time trap contract.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
