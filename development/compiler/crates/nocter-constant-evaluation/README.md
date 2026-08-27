# nocter-constant-evaluation

## Responsibility

Plan and evaluate the closed compile-time expression subset used by declaration and checking stages.

## Contract

The crate consumes syntax-owned constant expressions plus explicit semantic support and produces
typed constant values or source-backed evaluation failures. It does not perform general body
checking, runtime execution, name lookup, or target code generation.

## Invariants

- Evaluation order and supported operations follow the public constant contract.
- The evaluator receives resolved inputs instead of repeating declaration lookup.
- Failure cannot publish a partially evaluated semantic constant.
