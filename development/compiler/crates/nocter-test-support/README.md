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
