# nocter-constant-evaluation

## Responsibility

Own closed, syntax-independent plans and deterministic evaluation for compile-time expressions and
checked callable bodies.

## Contract

The crate consumes syntax-owned constant expressions plus explicit semantic support and produces
typed scalar constants, recursively frozen tuple and fixed-array values, checked-body callable recipes, or closed
callable plans, plus semantic evaluation failures. Recipes retain checked body identities,
store-relative types, and already-selected call targets; closed plans replace only those type edges
with evaluator-domain shapes. This crate never consumes a checked body directly. It does not
perform body checking, runtime execution, name lookup, overload selection, or target code
generation.

## Invariants

- Evaluation order and supported operations follow the public constant contract.
- The evaluator receives resolved inputs instead of repeating declaration lookup.
- Decimal floating evaluation receives syntax-owned suffix decomposition and a selected format;
  it never parses a suffix or asks the compiler host for a floating-point value.
- Failure cannot publish a partially evaluated semantic constant.
- Callable recipe and plan construction validate every node, parameter, and local edge before
  publication. A closed plan retains the exact type of every parameter and its result, so execution
  does not rely on its caller to pair values with an external signature.
- A callable plan contains no syntax or source coordinate and cannot request a semantic decision.
- One generic operation representation serves both checked recipes and closed plans. Its exhaustive
  call-target mapping is the only recipe-to-plan operation transform, so specialization cannot
  grow a second partial operation model.
- Callable specialization keys contain closed evaluator-domain type shapes rather than `TypeId`, so
  a plan identity cannot be paired with a sibling checked-program type store.
- One query owns every reachable closed callable specialization. Generic bodies are projected only
  under an exact argument set, and recursive source calls close as graph edges rather than being
  mistaken for plan-construction cycles.
- Evaluation limits count semantic plan operations and source-call depth, never host instructions
  or elapsed time; all callers use the same nonzero limit contract.
- One closed-plan executor owns its remaining budget and memoized call results. It validates input,
  local, node, and result shapes; recursive calls therefore cannot bypass the depth limit or make
  malformed values observable.
- Constant expressions and checked callable plans delegate arithmetic, comparison, conversion, and
  target floating behavior to one scalar operation implementation.
- One dependency query owns absent, active, and completed key states. It memoizes shared
  dependencies, reports exact active cycles, and removes failed active branches before returning.
- Each completed query value is allocated once and shared across every dependent request. The
  query has no `Clone` implementation, so a semantic construction cannot fork a second memoization
  authority or copy a large frozen aggregate on each dependency edge.
- Constant plans freeze reference edges once during planning. Evaluation requests those edges
  through the query and does not construct a separate dependency-order traversal.
