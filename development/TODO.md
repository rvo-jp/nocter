# Nocter Development Handoff

## Current State

Nocter v0.64.0 Application Encoding and Identity is published and externally audited. Annotated tag
`v0.64.0` resolves to publication commit `fd7df74721f29e8fa650b3527e94923321671839`, and the
public single asset matches the retained qualified candidate byte for byte.

## Next Work

Choose the next milestone from practical application needs. Preserve the newly established codec,
digest, UUID, and entropy ownership boundaries instead of adding parallel representation helpers.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.64.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
