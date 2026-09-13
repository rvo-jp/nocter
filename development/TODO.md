# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 implementation and Phase 7
release-readiness review are complete on `develop-v0.49.0`. Release-content commit
`63ce5e0c09175f0488ef03fbbf43131b848e8f12` passed the complete disposable compiler gate and
the deterministic two-build installed-package qualifier. The exact retained archive is ready for
publication. Structured blocking and asynchronous process I/O share one launch, endpoint,
exact-observation, and abandonment model. Website HTML is no longer a repository authority: the
generator requires an out-of-tree destination, and the main-branch workflow produces a
source-identified GitHub Pages artifact. No practical release-blocking finding remains.

## Next Work

Record qualification evidence, then create the separate publication-metadata commit that advances
the root download, specification status, and release index to v0.49.0. Publication is authorized:
fast-forward `main`, switch the repository Pages source to **GitHub Actions**, publish the exact
qualified archive under annotated tag `v0.49.0`, verify the public asset and source-identified Pages
deployment, and record the immutable audit without rebuilding or replacing the archive.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
