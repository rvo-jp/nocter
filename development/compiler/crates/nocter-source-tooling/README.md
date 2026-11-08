# nocter-source-tooling

## Responsibility

Provide source-only tooling that depends on syntax but not semantic compiler state.

## Contract

The crate formats Nocter source and derives lexical/syntactic token classifications from immutable
source and syntax products. Semantic highlighting, hover, navigation, and completion belong to
analysis and are not reconstructed here.

## Invariants

- Formatting preserves syntax and emits the canonical source style.
- Comment layout owns trivia ordering and anchoring independently of token layout. Formatting
  preserves exact comment text, comment kind, source order, and syntax-owned documentation
  attachment.
- Formatting preserves authored callable execution and guarantees in their canonical modifier
  positions.
- Source token classification never invents semantic identity.
- Released inspection/formatting retains its `SourceFile` goal. The future private core root is
  rejected before released formatting rules run; exposing or formatting that language is a separate
  migration boundary. Retired access-prefix assertion diagnostics have no formatting/inspection
  category or source-text reconstruction path.
- Invalid syntax remains representable and diagnosable after tooling operations.
