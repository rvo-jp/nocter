# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. The immutable evidence is recorded in
[`development/history/release-audits/v0.59.0.md`](history/release-audits/v0.59.0.md).

The post-release repository and documentation authority reviews are complete. Compiler integration
tests and reusable Nocter corpora have explicit physical owners. The distributed standard library
now lives at root `std/`, current cross-responsibility contracts live under
`development/architecture/`, and local test and measurement policy lives with its mechanism.
Findings and evidence are recorded in the
[`Repository Structure Review after v0.59.0`](history/reviews/repository-structure-after-v0.59.0.md)
and the
[`Repository Documentation Authority Review after v0.59.0`](history/reviews/repository-documentation-authority-after-v0.59.0.md).

The generated documentation site retains the complete Nocter hero on fragment-free external entry
URLs. All generated site-internal navigation enters at the content boundary, while specific heading
links remain exact. Global sections, local search, page contents, adjacent-page links, narrow-screen
navigation, and accessible hero tabs are derived from the published document set without a second
page registry or hosted search service.

v0.60.0 release preparation is active after completion of one allocation-free raw-DEFLATE engine,
concatenated gzip decoding, bounded POSIX ustar observation, shared blocking and asynchronous
transport cursors, the bounded `archive-inspect` application, editor qualification, and
whole-area review. Release identity now selects `0.60.0`; public candidate notes and the
release-preparation contract are authored. The completed implementation scope is recorded in
[`v0.60.0: Streaming Compression and Safe Archives`](history/milestones/v0.60.0.md).

## Next Work

Commit the exact release content, run the complete compiler and documentation gates from that clean
commit, then run deterministic two-build packaging and installed-home qualification. Record the
measured artifact identities only after qualification succeeds. Publication must reuse the
retained candidate archive without rebuilding it.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
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
