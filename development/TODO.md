# Nocter Development Handoff

## Current State

Nocter v0.21.0 is [published and externally audited](releases/v0.21.0.md). The exact source,
artifact, publication, and public re-download evidence is frozen; its tag and release asset must
not be replaced.

The active [v0.22.0 milestone](milestones/v0.22.0.md) adds a strict owning JSON DOM, exact number
tokens, parsing, and generation in ordinary standard-library source. Phases 0 through 4 are complete
and reviewed. The public contract, exact Number representation, lexical cursor, Unicode escape
foundation, typed failure channels, owning Value model, explicit-stack parser, shared String/Writer
generator, and practical application/tooling integration are closed; Phase 5 stabilization is the
next checkpoint.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Implement v0.22.0 Phase 5 from the adopted JSON contract and
`development/docs/json-implementation.md`:

- run adversarial RFC 8259, Unicode, exact-number, duplicate-name, deep-input, cleanup, and
  allocation qualification over the completed parser and generator;
- qualify the complete native, LSP, formatter, documentation, packaging, and installed-artifact
  surface without weakening tests around private authority boundaries;
- audit the complete `std/json` source graph for repeated grammar, traversal, ownership, failure,
  or representation decisions;
- remove the future-direction notice only after the implementation and installed release candidate
  pass the complete gate;
- prepare release evidence only after the Phase 5 review has no unresolved practical defect.

Do not add pretty printing, canonical member ordering, floating point, `char`, streaming JSON input,
public failure wrappers, or a compiler-known JSON declaration during Phase 5.

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
