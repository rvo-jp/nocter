# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State has entered release preparation after completing Phases 0–5 and their
implementation review. Release identity now selects v0.66.0; the clean release-content commit and
deterministic artifact qualification remain.

## Next Work

Commit the exact v0.66.0 release-content inputs, rerun the complete disposable compiler gate, and
run clean-tree double-generation qualification. Record the retained candidate evidence, then
publish that exact archive without rebuilding it and complete the external publication audit. The
[release-preparation record](history/milestones/v0.66.0-release-preparation.md) owns these gates.

Preserve the v0.65.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.65.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
