# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phase 0 has fixed the milestone's
responsibility boundaries, invariants, migration rule, and completion gates. v0.66.0 remains the
published and externally audited release boundary.

## Next Work

Implement Phase 1 as one ownership slice: add a target-backed non-waiting exclusive operation to the
file-owner contract, retain a stable sibling lock owner for the full `Store` lifetime, and prove
same-process, cross-process, close, and process-exit behavior. Do not encode ownership through
replaceable journal inode identity or persistent marker contents.

Preserve the v0.66.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.66.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
