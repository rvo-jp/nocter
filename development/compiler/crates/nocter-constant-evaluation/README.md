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
- Evaluation limits count semantic plan operations and source-call depth, never host instructions
  or elapsed time; all callers use the same nonzero limit contract.
- One dependency query owns absent, active, and completed key states. It memoizes shared
  dependencies, reports exact active cycles, and removes failed active branches before returning.
- Constant plans freeze reference edges once during planning. Evaluation requests those edges
  through the query and does not construct a separate dependency-order traversal.
