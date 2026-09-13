# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 implementation and Phase 7
release-readiness review are complete on `develop-v0.49.0`; its sole release identity and public
candidate notes now name v0.49.0. Structured blocking and asynchronous process I/O share one
launch, endpoint, exact-observation, and abandonment model. Website HTML is no longer a repository
authority: the generator requires an out-of-tree destination, and the main-branch workflow
produces a source-identified GitHub Pages artifact. No practical release-blocking implementation
finding remains.

## Next Work

Commit the v0.49.0 release content, run the complete disposable compiler gate, then run the
identity-gated two-build deterministic local qualifier from that clean commit. Record the exact
archive, installed home, checksums, release notes, and publication boundary. Publication is already
authorized for this release: fast-forward `main`, switch the repository Pages source to **GitHub
Actions**, publish the exact qualified archive under annotated tag `v0.49.0`, verify the public
asset and Pages deployment, and record the immutable audit without rebuilding the archive.

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
