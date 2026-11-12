# nocter-test-support

## Responsibility

Provide shared fixture construction and public-example catalogs for compiler tests without entering
production dependency paths.

## Contract

The crate creates validated test sources, packages, compile inputs, and runtime contract fixtures.
Tests that consume physical repository inputs obtain the repository root, authored standard-library
root, and release identity from this crate rather than reconstructing paths or copying the release
number. One public-example run contract owns its relative arguments, exact environment, standard
input, expected process result, fixtures, and filesystem postconditions; source-tree and
installed-home runners consume that same data. It does not define language behavior or bypass
production validation on behalf of tests.

## Invariants

- Public examples are discovered through one canonical catalog.
- Every public-example runner applies the catalog's complete invocation contract rather than
  supplying runner-local arguments or environment policy.
- Physical standard-package tests do not copy the active release number or repository traversal.
- Test-only convenience cannot manufacture an accepted production product that normal APIs reject.
- The crate remains a development dependency.

`ValueSourceFixture` owns exact source and production private-goal syntax for value-binding tests.
It can load the shared physical corpus through the repository-root authority. It creates no
accepted declaration/body/unit product; integration tests must use the normal production APIs.
`ValueModuleFixture` similarly owns exact physical sources and explicit module/see/use selections,
without discovery or semantic acceptance. The shared `qualified_namespace_fixture` loads the one
module corpus under `development/compiler/tests/fixtures/value-modules/qualified-namespaces`.
`trusted_error_module_fixture` similarly selects the authored `trusted-errors` corpus and its exact
host-provided role profile; source names alone confer no authority. Both loaders use one physical
corpus ingestion helper. Checking, required execution and native consumers use those same files. Semantic preparation remains in
the consuming test, rather than becoming a second compiler in test support.

`integer_failure_sources` supplies identical runtime-admissible arithmetic bodies to required and
native execution tests. Its boundary inputs are fixture data, not a replacement for the numeric
policy; consumers assert the failure at their own production boundary.
