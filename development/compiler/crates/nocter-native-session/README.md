# nocter-native-session

## Responsibility

Compose a successful compiler session with Target, MIR, Machine, ARM64, and Mach-O lowering and
return a complete native artifact or typed failure.

## Contract

The crate accepts only a query-backed `CompiledTarget`, orchestrates backend stage contracts, and
owns output bytes plus native test execution support. It does not discover source, run semantic
analysis, implement backend rules, select command-line policy, or publish filesystem artifacts.

## Internal Responsibilities

- `src/tests.rs` owns only the shared native-session test harness: discovery, compilation, temporary
  package lifecycle, and generated-image execution.
- `src/tests/` groups integration tests by the runtime or compiler boundary they exercise. One test
  group cannot use another group's private helpers.
- reusable Nocter source corpora live under `development/compiler/tests/fixtures/native/`; Rust
  test modules select those inputs but do not embed large source packages.

## Invariants

- Each stage consumes exactly the previous closed product.
- Native requests cannot carry a discovery snapshot or reopen semantic compilation.
- Multi-executable lowering receives already paired identities and executable programs from
  session; it cannot repeat target lookup or combine separate target owners.
- A backend integrity failure cannot be presented as a source-language diagnostic.
- Partial native output is never returned as a successful artifact.
