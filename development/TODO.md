# Nocter Development Handoff

## Current State

Nocter v0.21.0 is [published and externally audited](releases/v0.21.0.md). The exact source,
artifact, publication, and public re-download evidence is frozen; its tag and release asset must
not be replaced.

The active [v0.22.0 milestone](milestones/v0.22.0.md) adds a strict owning JSON DOM, exact number
tokens, parsing, and generation in ordinary standard-library source. Phases 0 through 5 are complete
and reviewed. The public contract, implementation, practical integration, adversarial JSON matrix,
shared collection failure policy, standard dependency audit, and public contract documentation are
closed. Release preparation is the next checkpoint.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Prepare the v0.22.0 release candidate without changing the completed JSON or standard-library
contract:

- align the release version, standard package identity, manifest, release notes, and public links;
- run two independent complete source qualifications and warnings-denied Clippy gates;
- generate two deterministic optimized artifacts and compare both archives and installed homes;
- qualify a fresh installed home across version, doctor, help, init, offline check/test/graph,
  native run/build, direct execution, and framed LSP requests;
- remove the JSON future-direction notice only after that installed candidate passes;
- freeze candidate hashes and evidence, then wait for explicit publication authorization.

Do not add pretty printing, canonical member ordering, floating point, `char`, streaming JSON input,
public failure wrappers, or a compiler-known JSON declaration during release preparation.

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
