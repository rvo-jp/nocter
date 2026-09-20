# Nocter Development Handoff

## Current State

Nocter v0.63.0 Cryptographic Randomness is published and externally audited. v0.64.0 Application
Encoding and Identity has completed hexadecimal, Base64, SHA-256, UUID, and URL-safe random-token
contracts and is now in Phase 4 adoption, review, and qualification. Release inputs remain on
v0.63.0 while the new standard-library contracts are developed.

## Next Work

Add practical public adoption, review the complete responsibility and behavior surface, and run
the full qualification matrix before release preparation.

Preserve the v0.63.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No release blocker remains.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
