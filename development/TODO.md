# Nocter Development Handoff

## Current State

Nocter v0.55.0 is published and externally audited. v0.56.0 Phases 0–4 are complete: `from` now
uses one value-provenance contract across results, inputs, receivers, annotated locals, structural
callables, calls, closures, and editor presentation. Separate synchronous and asynchronous lending
iteration contracts retain the active receiver loan through generic associated items. Borrowed
scan windows, reusable asynchronous byte/text windows, and HTTP query decoding now exercise that
contract without hidden ownership copies. The active scope and completion gates live in
[`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Begin Phase 5 with canonical editor presentation for the completed contract positions, then use
the zero-copy parsing and protocol-decoding surface in one practical application. Finish with the
complete release gate and adversarial authority review.

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
