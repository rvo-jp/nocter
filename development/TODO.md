# Nocter Development Handoff

## Current State

Nocter v0.18.0 is [published and externally audited](releases/v0.18.0.md). Its Phase 0 through Phase
3 changes are implemented and reviewed, and the exact source, artifact, publication, and public
re-download evidence is frozen. The `v0.18.0` tag and release asset must not be replaced.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
`development/milestones/v0.18.0.md` and `development/reviews/` rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Define the next milestone from concrete language or practical-API needs before changing the
compiler or standard library. No post-v0.18.0 implementation phase is active.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
