# nocter-test-support

## Responsibility

Provide shared fixture construction and public-example catalogs for compiler tests without entering
production dependency paths.

## Contract

The crate creates validated test sources, packages, compile inputs, and runtime contract fixtures.
Tests that consume physical repository inputs obtain the repository root, authored standard-library
root, and release identity from this crate rather than reconstructing paths or copying the release
number. It does not define language behavior or bypass production validation on behalf of tests.

## Invariants

- Public examples are discovered through one canonical catalog.
- Physical standard-package tests do not copy the active release number or repository traversal.
- Test-only convenience cannot manufacture an accepted production product that normal APIs reject.
- The crate remains a development dependency.
