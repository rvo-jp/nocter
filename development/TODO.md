# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State is active; Phase 0 established the durable filesystem authority.

## Next Work

Implement v0.66.0 Phase 1: define one bounded, versioned, length-delimited, checksummed journal
record; append and synchronize complete records; replay them deterministically; accept only an
incomplete final record; and reject earlier corruption. Reuse the existing file lifecycle, binary
codec, checksum, and durable replacement authorities rather than creating parallel implementations.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
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
