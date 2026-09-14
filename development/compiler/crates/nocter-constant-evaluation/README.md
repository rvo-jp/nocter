# nocter-constant-evaluation

## Responsibility

Own closed, syntax-independent plans and deterministic evaluation for compile-time expressions and
checked callable bodies.

## Contract

The crate consumes syntax-owned constant expressions plus explicit semantic support and produces
typed scalar constants, recursively frozen values, or validated callable plans, plus semantic
evaluation failures. Callable plans retain checked body identities and already-selected call
targets; this crate never consumes a checked body directly. It does not perform body checking,
runtime execution, name lookup, overload selection, or target code generation.

## Invariants

- Evaluation order and supported operations follow the public constant contract.
- The evaluator receives resolved inputs instead of repeating declaration lookup.
- Decimal floating evaluation receives syntax-owned suffix decomposition and a selected format;
  it never parses a suffix or asks the compiler host for a floating-point value.
- Failure cannot publish a partially evaluated semantic constant.
- Callable-plan construction validates every node, parameter, and local edge before publication.
- A callable plan contains no syntax or source coordinate and cannot request a semantic decision.
