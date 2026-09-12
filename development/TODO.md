# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 implementation and Phase 7
release-readiness review are complete on `develop-v0.49.0`. Structured blocking and asynchronous
process I/O share one launch, endpoint, exact-observation, and abandonment model. A fully disposable
compiler gate and an optimized installed-package smoke test pass. No practical release-blocking
implementation finding remains. Website HTML is no longer a repository authority: the generator
requires an out-of-tree destination, and the main-branch workflow produces a source-identified
GitHub Pages artifact.

## Next Work

Begin v0.49.0 release preparation. Change the sole release identity to `0.49.0`, align generated
release-facing documentation and notes, and run the identity-gated two-build deterministic local
qualifier. Audit the resulting archive, installed home, checksums, release notes, and publication
assets. When cutting `main` over, select **GitHub Actions** as the repository's Pages source before
running the new documentation deployment; do not leave the site configured for the removed
`docs/` directory. Stop before tagging, pushing, uploading, merging, or publishing until explicitly
authorized.

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
