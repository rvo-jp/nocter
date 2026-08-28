# Nocter Development Handoff

## Current State

Nocter v0.17.0 is published and externally audited. The v0.18.0 Phase 0 through Phase 3 changes are
implemented and reviewed, and [release preparation](milestones/v0.18.0-release-preparation.md) is
active. The working tree uses the v0.18.0 release-candidate language contract without a
compatibility mode.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
`development/milestones/v0.18.0.md` and `development/reviews/` rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

1. Freeze the release-content commit after validating version identity, public notes, generated
   documentation, and repository integrity.
2. Run the two clean external-target source qualifications, explicit public-HTTPS test,
   deterministic artifact assembly, and fresh-install matrix.
3. Record exact candidate evidence and perform the final boundary review before publication.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
