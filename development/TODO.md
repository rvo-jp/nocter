# Nocter Development Handoff

## Current State

Nocter v0.18.0 is [published and externally audited](releases/v0.18.0.md). Its Phase 0 through Phase
3 changes are implemented and reviewed, and the exact source, artifact, publication, and public
re-download evidence is frozen. The `v0.18.0` tag and release asset must not be replaced.

The [v0.19.0 milestone](milestones/v0.19.0.md) is active. Phase 0 filesystem traversal and Phase 1
streaming text input are complete and reviewed. Their public stream, UTF-8 and error policy,
close-once ownership, bounded storage, native behavior, and editor coverage are now the accepted
basis for Phase 2.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
`development/milestones/v0.18.0.md` and `development/reviews/` rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Specify v0.19.0 Phase 2 collection ordering. Fix the in-place slice API, `<` requirement, move-only
element behavior, deterministic ordering contract, failure model, and scratch-storage bound before
implementation begins. `Vec<T>` must reuse the slice implementation through coercion rather than
owning a second algorithm.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
