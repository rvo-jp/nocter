# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. Canonical variable-width codecs, CRC-32,
integrity-checked binary records, native and editor integration, deterministic packaging, the
public asset, and the Pages deployment are complete. The immutable evidence is recorded in
[`development/history/release-audits/v0.59.0.md`](history/release-audits/v0.59.0.md).

## Next Work

Select the next milestone from concrete application and standard-library pressure. Do not reopen
v0.59.0; any correction requires a new version, implementation gate, qualified archive, tag, and
publication audit.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
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
