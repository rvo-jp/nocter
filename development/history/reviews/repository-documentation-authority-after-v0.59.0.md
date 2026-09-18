# Repository Documentation Authority Review after v0.59.0

## Scope

This review classifies repository documentation by audience, authority, lifecycle, and the product
source it describes. It follows the physical test-ownership review and replaces no public language
or standard-library behavior.

## Findings

1. `development/std/` mixed a distributed product package and public API documentation into the
   contributor-only tree. The documentation site compensated with a repository-to-public path
   projection.
2. `development/design/` placed 34 current contracts, implementation designs, test policy,
   performance methodology, maintenance workflow, and phase-shaped records in one flat directory.
3. File, datagram, and HTTP client contracts remained split by their historical delivery phases
   after their implementations had acquired one shared current authority.
4. Compiler integration tests reconstructed the physical standard-library path independently from
   each consuming crate.

## Remediation

- The authored and distributed standard-library package now lives at root `std/`. Its repository
  path, public documentation path, package input, and release-packaging input are the same.
- Current cross-responsibility contracts now live under `development/architecture/`, grouped by
  pipeline handoff, semantic contract, representation, runtime integration, and standard-library
  implementation.
- Grammar coverage moved beside compiler corpora, performance methodology moved beside the
  benchmark runner, and maintenance workflow moved to the contributor entry point.
- Current filesystem, networking, and HTTP client documents now describe the unified authority
  instead of retaining separate phase documents.
- `nocter-test-support` is the sole repository-root and authored-standard-library path authority for
  compiler integration tests.

## Authority Result

The resulting physical boundaries are:

- `spec/`: public language, platform, and tooling semantics;
- `std/`: distributed standard-library source, exact declarations, and observable behavior guides;
- `development/architecture/`: current contracts spanning implementation responsibilities;
- crate README files: one crate's local mechanism and exported contract;
- local development directories: their own test, benchmark, packaging, site, generation, and
  verification procedures;
- `development/history/`: non-current work order, findings, alternatives, and qualification
  evidence.

No compatibility path remains for `development/std/` or `development/design/`.

## Verification

- documentation generation tests and an out-of-tree website build;
- warnings-denied Clippy for every compiler crate whose repository input changed;
- compiler analysis, discovery, command, language-server, workspace-analysis, and native-session
  tests that consume the physical standard library;
- release packaging input validation after Git records the root `std/` tree;
- formatting and whitespace validation.
