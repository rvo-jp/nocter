# Nocter Development Handoff

## Current State

Nocter v0.22.0 is [published and externally audited](releases/v0.22.0.md). Its strict owning JSON
DOM, exact number tokens, parsing, generation, practical integration, adversarial qualification,
shared collection failure policy, standard dependency audit, and public contract documentation are
complete. The exact source, artifact, publication, and public re-download evidence is frozen. The
`v0.22.0` tag and release asset must not be replaced.

The active compiler is under `development/compiler/`. The v0.24.0 explicit-value-subject migration
is complete and reviewed: selected imports are type-only, external functions and constants retain
a module subject, namespace aliases are first-class, and constants require `UPPER_SNAKE_CASE`.
Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Prepare the qualified [v0.24.0](milestones/v0.24.0.md) candidate for release only when publication
is requested.

Do not cache `NodeId`, `SourceId`, frontend bindings, or `SourceIndex` as if they were reusable
semantic programs. Stable declaration identities, module-local semantic queries, feature-demand
editor analysis, cancellation, parallelism, and persistent caches remain later work.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
