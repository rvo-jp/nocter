# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 Phase 0 is complete: `std/bytes` owns one
ordinary generic `PrefixDecode<T>` declaration for decoded values and widths, incomplete input,
overflow, and non-canonical representations. Native and editor fixtures prove that execution,
hover, completion, navigation, and semantic highlighting consume that same public identity. The
active scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Complete portable signed fixed-width bit conversion and canonical unsigned base-128 and ZigZag
codecs on the Phase 0 result. Keep numeric conversion rules unchanged, reject every invalid prefix
before cursor mutation, and preserve output bytes until the complete variable-width representation
fits.

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
