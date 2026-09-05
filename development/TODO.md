# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. The public tag resolves to publication commit
`8678b994955a5599a67fa3941e2b16db766123ff`, and the public archive matches the qualified local
candidate byte for byte.

## Next Work

No milestone is active. Preserve the immutable v0.36.0 tag and asset. Define a new version before
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
