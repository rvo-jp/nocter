# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. The public tag resolves to publication commit
`507958b5a7ae648400a64f117910624c3831408f`, and the public archive matches the qualified local
candidate byte for byte.

## Next Work

No milestone is active. Preserve the immutable v0.38.0 tag and asset. Define a new version before
changing released behavior or distribution content.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
