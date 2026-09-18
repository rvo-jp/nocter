# Repository Structure Review after v0.59.0

Date: 2026-09-19

## Conclusion

The repository has one coherent top-level structure and no unresolved placement defect that
justifies a broad directory migration. User documentation, language specification, runnable
examples, release records, compiler development, and generated state have distinct owners. The
review found one repeated source-layout defect inside the compiler: several production modules
embedded integration suites substantially larger than the responsibility being implemented. Those
suites and their reusable Nocter inputs are now physically separate without introducing a new API
or changing behavior.

## Reviewed Boundaries

The review inspected:

- every tracked repository-root entry and every direct `development/` responsibility;
- specification, example, release, design, history, standard-library, packaging, site, Unicode,
  benchmark, and verification placement;
- all 53 Cargo workspace members, their colocated responsibility documents, and production
  dependency edges;
- the largest Rust source files and every inline test module larger than 300 lines;
- reusable syntax, standard-library, and native compiler test corpora;
- generated website publication boundaries and structural navigation; and
- ignored workspace caches, release artifacts, and generated documentation paths.

## Remediation

### Native-session tests

The previous `nocter-native-session/src/tests.rs` mixed the shared compiler/execution harness,
large embedded Nocter packages, and tests for callables, processes, files, networking, asynchronous
execution, recoverable allocation, and semantic recovery in one file.

The shared file now owns only discovery, compilation, temporary-package, fixture-selection, and
generated-image execution mechanisms. Responsibility-specific suites live under `src/tests/`.
Substantial static Nocter packages live under `development/compiler/tests/fixtures/native/session/`
instead of escaped Rust strings. A test group can consume the shared harness but cannot depend on
another group's private helpers.

### Language-server tests

`server.rs` and `server/semantic_requests.rs` previously placed thousands of integration-test lines
after their production implementations. Production routing and semantic request projection now
remain in those files, while the unchanged tests live in sibling modules under `server/`. The move
did not add test hooks or widen production visibility.

### Compiler orchestration tests

The declaration-lowering pipeline and workspace-analysis entry had the same pattern: cross-stage or
cross-generation integration suites were larger than their production entry modules. Those suites
now live in `pipeline/tests.rs` and `src/tests.rs`, respectively. Focused unit tests remain beside
the local responsibilities they inspect.

## Structures Deliberately Retained

- The 53 Cargo crates are not grouped or merged by size. Narrow crates such as semantic-product,
  language vocabulary, toolchain contracts, target selection, and standard profiles are capability
  boundaries. Architecture tests constrain their dependency sets and reviewed consumers; merging
  them would weaken information hiding without removing duplicated computation.
- `development/design/` remains one cataloged directory. Its documents own cross-crate contracts,
  while crate-private mechanisms remain in colocated crate READMEs. Adding thematic subdirectories
  would change many stable links without creating a missing authority boundary.
- `development/history/` remains large by file count but small as authored source and is explicitly
  excluded from current semantics and website guidance. Milestones, reviews, release audits, and
  legacy design already have distinct catalogs and meanings.
- Standard-library module sources remain under `development/std/`, with each `index.nct` owning the
  checked public surface and each assigned behavior guide owning its longer observable subject.
- Large focused test-only files are not divided merely to satisfy a line threshold. A further split
  requires an actual second responsibility or a helper contract crossing unrelated test domains.

## Generated and Local State

`development/compiler/target/`, `dist/`, `docs/`, `.nocter/`, and root `target/` are ignored and are
not repository authorities. The review observed that local Cargo caches and retained release
archives account for nearly all physical checkout size. They were not deleted because this phase
reviewed authored structure rather than destroying the developer's cache or retained artifacts.
The documented `cargo clean --manifest-path development/compiler/Cargo.toml` command remains the
explicit way to reclaim the workspace cache.

## Verification

- `cargo test -p nocter-native-session --lib --quiet`: 98 passed under real localhost and generated
  binary execution conditions.
- `cargo test -p nocter-language-server --lib --quiet`: 117 passed.
- `cargo test -p nocter-declaration-lowering -p nocter-workspace-analysis --lib --quiet`: 157
  passed.
- `development/verification/verify-compiler.sh`: passed from a disposable external Cargo target
  after the native-session and language-server restructuring, including warnings-denied Clippy,
  workspace tests, no-default-features checking, and rustdoc. The later declaration-lowering and
  workspace-analysis moves were test-module-only and passed their focused 157-test gate.
- `node development/site/test-generation.js`: passed.
- `node development/site/build-docs.js --output /tmp/nocter-repository-structure-site`: generated
  the complete site outside the repository.
- `node development/verification/verify-repository-metadata.js`: verified 53 Cargo packages.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

## Remaining Findings

None. This conclusion is limited to current, observable repository structure. It does not impose a
line-count rule or predict hypothetical future responsibilities.
