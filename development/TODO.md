# Nocter Development Handoff

## Current State

Nocter v0.61.0 is published and externally audited. v0.62.0 Structured Command-Line Applications
has started with a contract-first `std/cli` boundary over the existing raw `std/process` argument
authority.

## Next Work

Complete the v0.62.0 schema, parser, presentation, nested-command, and application-adoption phases.
Keep parsing, result lookup, and help generation dependent on one immutable schema; do not add
compiler-known CLI declarations, hidden process termination, or a second raw-argument authority.

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
