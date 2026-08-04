# Construction Surfaces

This document owns the compiler design for the adopted v0.3.0 `construct` declaration. Public
semantics are defined by [Construction Surfaces](../../spec/19-construction-surfaces.md).

## Compiler-Owned Model

The resolver attaches one `ConstructionSurface` to each nominal `TypeSymbol`. It contains resolved
entries for structural construction, typed literals, associated construction functions, enum
variants, and the optional default identity. An entry retains its declaration identity, exact focus
span, visibility, specialized signature, documentation source, and entry kind.

The surface is not an LSP index and is not reconstructed from exported names. Imports clone or
specialize the target type symbol with the same construction identities. Presentation receives a
visible owner spelling separately from canonical identity.

## Phase Ownership

| Responsibility | Owner |
|---|---|
| `construct` and contextual `default` grammar | `parser/constructs` |
| declaration/member source model | `ast/constructs` |
| target, uniqueness, visibility, result, and default validation | `resolve/constructions` |
| callable and literal body facts | existing callable/literal typecheck and ownership passes |
| callable and literal native lowering | existing callable/literal IR entry points |
| normalized construction presentation | `analysis/constructions` and `analysis/presentation` |
| protocol conversion only | `driver/lsp` |

AST visitors must enter construct member bodies explicitly. They must not flatten a construct into
synthetic top-level declarations: flattening loses ownership, block range, default identity, and
document-symbol hierarchy. Existing function and literal validation/lowering should operate on
borrowed construct members through shared iterators rather than duplicate their rules.

## Invariants

- One construct declaration owns one exact nominal target in its defining module.
- Every public construction function and literal has one construction entry and one declaration
  identity.
- A member body is resolved, checked, lowered, and indexed exactly once.
- Default selection refers to an entry identity, never a display name or source string.
- Structural-construction accessibility is computed by the resolver and consumed by typecheck.
- Editor features query the resolved surface and never search for names such as `new`, `create`, or
  `from_*`.
- Removed top-level construction forms receive migration diagnostics; no compatibility surface is
  retained.

## Acceptance Boundary

The implementation is complete when parser and diagnostic tests cover malformed blocks and every
invariant; distributed standard-library tests build and run `String` and `Vec<T>` through construct
members; raw external struct construction follows default selection; and hover, completion,
signature help, definition, references, semantic tokens, and document symbols agree on the same
member identities and exact spans.
