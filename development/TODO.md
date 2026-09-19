# Nocter Development Handoff

## Current State

Nocter v0.61.0 is published and externally audited. v0.62.0 Structured Command-Line Applications
is implementation-complete and qualified. It provides one schema authority for parsing and help,
owned parsed results, nested commands and repeated options, and structured arguments throughout
every public argument-consuming example.

## Next Work

Prepare v0.62.0 for release as a separate change. Freeze the qualified implementation, select the
exact release identity, generate and inspect the distribution from that identity, run the formal
double-generation qualification, and publish only the resulting verified artifact.

Preserve the v0.61.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.62.0 release preparation.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
