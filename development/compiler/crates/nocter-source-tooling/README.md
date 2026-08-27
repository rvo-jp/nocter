# nocter-source-tooling

## Responsibility

Provide source-only tooling that depends on syntax but not semantic compiler state.

## Contract

The crate formats Nocter source and derives lexical/syntactic token classifications from immutable
source and syntax products. Semantic highlighting, hover, navigation, and completion belong to
analysis and are not reconstructed here.

## Invariants

- Formatting preserves syntax and emits the canonical source style.
- Source token classification never invents semantic identity.
- Invalid syntax remains representable and diagnosable after tooling operations.
