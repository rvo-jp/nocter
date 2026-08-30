# Nocter Development Handoff

## Current State

Nocter v0.21.0 is [published and externally audited](releases/v0.21.0.md). The exact source,
artifact, publication, and public re-download evidence is frozen; its tag and release asset must
not be replaced.

The active [v0.22.0 milestone](milestones/v0.22.0.md) adds a strict owning JSON DOM, exact number
tokens, parsing, and generation in ordinary standard-library source. Phases 0 through 5 are complete
and reviewed. The public contract, implementation, practical integration, adversarial JSON matrix,
shared collection failure policy, standard dependency audit, and public contract documentation are
closed. Release candidate qualification is complete, and the retained artifact is frozen pending
explicit publication authorization.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Wait for explicit publication authorization. When authorized:

- reuse `dist/nocter-v0.22.0-arm64-darwin.tar.gz` without rebuilding it;
- advance the root README and public release index from v0.21.0 to v0.22.0;
- commit publication metadata, create and push an annotated `v0.22.0` tag, and upload the retained
  archive;
- download the public asset into a fresh temporary directory and compare it byte for byte with the
  qualified candidate;
- record public release, tag, digest, latest-release, and re-download evidence.

Do not add pretty printing, canonical member ordering, floating point, `char`, streaming JSON input,
public failure wrappers, or a compiler-known JSON declaration while the candidate is frozen.

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
