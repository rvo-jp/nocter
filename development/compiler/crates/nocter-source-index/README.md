# nocter-source-index

## Responsibility

Own the immutable bidirectional projection between already selected semantic identities and exact
source/syntax origins used by diagnostics and editor queries.

## Contract

Lowering and checking extend one builder beside their semantic products. Analysis consumes the
finished projection only after validating it against the matching generation. The crate does not
resolve names, choose visible declarations, type expressions, or implement editor fallback policy.

## Internal Responsibilities

- semantic entity to declaration/reference origins
- source coordinate to semantic bindings
- documentation ownership
- editor-visible spelling projections
- explicit projection-integrity issues

## Invariants

- Conflicting projections are retained as issues, never resolved by insertion order.
- Consumers validate the complete projection through one result contract; its internal issue list
  is not a cross-crate inspection API.
- One semantic identity may deliberately have multiple declaration origins, as with a capability
  fact derived from multiple authored prerequisites.
- A source range is not a semantic identity.
- Projection failure cannot change semantic program success.
- Raw projections cannot cross directly into protocol code.
