# Nocter Development Handoff

## Current State

Nocter v0.61.0 is published and externally audited. v0.62.0 Structured Command-Line Applications
has completed its standard-library contract and migrated every public argument-consuming example
from raw process indexes to explicit schemas. Final responsibility review and qualification remain.

## Next Work

Review and qualify v0.62.0. Confirm that parsing, result lookup, and help generation depend on one
schema, every accepted token is classified once, raw operating-system argument ownership remains
in `std/process`, and no hidden output or termination policy entered `std/cli`.

Preserve the v0.61.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.61.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
