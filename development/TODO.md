# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 Phases 0 and 1 are complete. `std/bytes`
owns one typed prefix result, exact signed fixed-width codecs, canonical unsigned LEB128, and
ZigZag-signed LEB128. Explicit integer `from_bits` and `to_bits` preserve two's-complement
representations without weakening value-preserving `as`. Native execution covers signed minima,
all prefix failure classes, and transactional short output. The active scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Add the independent `std/checksum` CRC-32 authority. Keep its state transition allocation-free,
derive or verify one canonical table from the published polynomial, and prove the standard check
value, empty input, arbitrary chunk boundaries, and repeatable finalization. Do not attach framing,
transport, or authentication meaning to the checksum module.

Preserve the v0.58.0 tag, release asset, public notes, specification snapshot, and publication audit
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
