# Nocter Development Handoff

## Current State

Nocter v0.55.0 is published and externally audited. v0.56.0 implementation is complete: `from` now
uses one value-provenance contract across results, inputs, receivers, annotated locals, structural
callables, calls, closures, and editor presentation. Separate synchronous and asynchronous lending
iteration contracts retain the active receiver loan through generic associated items. Borrowed
scan windows, reusable asynchronous byte/text windows, and HTTP query decoding now exercise that
contract without hidden ownership copies. The completed scope and gates live in
[`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Prepare v0.56.0 for release as a separate change. Update exact release identity, assemble and
qualify the installed artifact, write public release notes, and publish only after the preparation
commit is independently clean and the user explicitly requests publication.

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
