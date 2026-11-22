# nocter-native-session

## Responsibility

Compose a successful compiler session with Target, MIR, Machine, ARM64, and Mach-O lowering and
return a complete native artifact or typed failure.

## Contract

The crate accepts either a query-backed released target request or a checked v2 package session
and an executable selection. It orchestrates backend stage contracts and owns output bytes and native test execution
support. The [process-image handoff](../../../architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
defines which v2 entries reach native code. Source discovery, semantic analysis, backend rules,
command-line policy and artifact publication belong to their respective stages.

## Internal Responsibilities

- `src/tests.rs` owns only the shared native-session test harness: discovery, compilation, temporary
  package lifecycle, and generated-image execution.
- `src/tests/` groups integration tests by the runtime or compiler boundary they exercise. One test
  group cannot use another group's private helpers.
- `src/tests/private_values.rs` exercises the private front-end/MIR entry into the shared Machine,
  ARM64 and image-writing consumers. It does not extend the public session API; the
  [private handoff](../../../architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
  owns its admitted source and native qualification limits.
  Trusted primitive fixtures supply an explicit module authority to the same staged `SourceUnit`
  coordinator, including required header values. Selected module sources also use the common
  `ModuleSourceUnit` and the same Machine/ARM64/image consumers; the shared qualified-namespace
  corpus checks original call/constant/static targets, record fields and lexical imports through
  closure bodies and required array lengths in a real native process. A shared trusted-error module
  corpus selects same-named primitives from different logical modules and executes owned graph
  construction, context attachment and release, including qualified function-value calls.
  The query-session corpus additionally exercises the production v2 package selection and image
  entry. The root and child modules each declare their own `main` and execute the selected result.
  A fallible entry with a contextual error verifies the process exit status and stderr bytes.
  A selected child reads the actual argument count through the trusted primitive and the shared
  native process context. Its image runs with different argument vectors without recompilation.
  Two same-code factory closures from a different module retain distinct captured values through
  required evaluation and native execution. Native compilation shares the query-owned checked Unit.
  A header-only query cannot replace source acceptance.
- Native-specific source corpora live under `development/compiler/tests/fixtures/native/`.
  Cross-consumer module corpora live under `development/compiler/tests/fixtures/value-modules/` and are selected by
  `nocter-test-support`. Rust tests do not duplicate their authored source packages.

## Invariants

- Each stage consumes exactly the previous closed product.
- Native requests cannot carry a discovery snapshot or reopen semantic compilation.
- Multi-executable lowering receives already paired identities and executable programs from
  session; it cannot repeat target lookup or combine separate target owners.
- A backend integrity failure cannot be presented as a source-language diagnostic.
- Partial native output is never returned as a successful artifact.
