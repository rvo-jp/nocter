# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is published and externally audited. v0.69.0
Canonical Declaration Contracts is active. It first closes the existing modifier grammar and
compiler authority before any new modifier is considered. Linux target work remains deliberately
deferred until this source contract is stable.

## Next Work

Replace declaration-specific modifier lookahead with one syntax-owned callable-prefix recognizer in
v0.69.0 Phase 1. Preserve dedicated syntax nodes, the sole declaration-lowering guarantee
projection, and independent deferred-execution identity. Do not add new modifier vocabulary during
this work.

Preserve the v0.68.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. Phase 0 corrected the normative grammar drift around the implemented `const`
callable prefix.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
