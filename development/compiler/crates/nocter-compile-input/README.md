# nocter-compile-input

## Responsibility

Own the closed, syntax-backed compilation input shared by declaration lowering, checking, and
target validation.

## Contract

The crate packages reached sources and syntax trees, package/module identity, dependency edges,
selected target facts, toolchain bindings, and runtime contract input into immutable values. It does
not discover files or perform semantic lowering.

## Invariants

- Every source, syntax tree, module, and dependency edge belongs to one compile unit.
- Target selection is supplied as one completed authority and is never recomputed downstream.
- Dependency identities are canonical; display names and paths cannot substitute for them.
- Directly constructed test inputs obey the same validation boundary as production inputs.
