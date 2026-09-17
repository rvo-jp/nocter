# Nocter Development Handoff

## Current State

Nocter v0.56.0 has a qualified retained release candidate. The unified value-provenance, lending
API, and zero-copy standard-library milestone, reopened presentation review, whole-repository
compiler gate, deterministic two-build packaging, and installed-home qualification are complete.
The completed scope lives in
[`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Publish v0.56.0 from the retained `dist/nocter-v0.56.0-arm64-darwin.tar.gz` archive without
rebuilding it. Update published-version surfaces, commit them, create the annotated tag at that
publication commit, push `main` and the tag, create one non-draft non-prerelease GitHub Release,
then audit the public asset, tag, latest-release endpoint, installed home, and Pages deployment.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
