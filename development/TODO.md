# Nocter Development Handoff

## Current State

Nocter v0.63.0 Cryptographic Randomness is published and externally audited. Its annotated tag,
single release asset, latest-release endpoint, downloaded installed home, Compiler and
Documentation workflows, and source-identified Pages deployment all match the qualified source
and artifact.

## Next Work

Define the next milestone from concrete language or standard-library needs before changing release
inputs. Preserve the v0.63.0 source and artifact identities as immutable evidence.

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
