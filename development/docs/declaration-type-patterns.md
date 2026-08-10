# Declaration Type Pattern Architecture

Public syntax and semantics are specified in
[Generics, Interfaces, and Methods](../../spec/08-generics-interfaces-embedding-methods.md). This
document owns the compiler architecture for v0.11.0 Phase 5.

## Boundary

Only `instance` and `conform` headers are declaration type patterns. A pattern slot introduces a
bare binder; it does not accept an arbitrary type expression. `func`, `method`, nominal type,
interface, construction, coercion, and literal generic lists remain explicit name declarations.

The parser derives one ordered `GenericParamList` from first occurrences in pattern slots. This is
not a synthetic, spanless list: each parameter points to its first authored occurrence, and later
occurrences resolve to the same declaration identity.

## Refinement

The AST stores `where T = Type` as a directed binder refinement, not as symmetric type equality.
Validation requires the left name to identify a pattern binder, forbids a second refinement of the
same binder, and rejects a right side that recursively mentions the refined binder. Projection
equality remains a separate predicate with its existing symmetric relation semantics.

One normalized substitution map applies refinements after receiver/interface pattern matching.
All consumers use that service; no consumer scans a `where` clause or presentation string to
rediscover refinements.

## Coherence

Pattern overlap is satisfiability, not textual equality. Binder names are alpha-renamed, repeated
positions add equality constraints, and refinements contribute their normalized concrete shape.
Two conformances conflict when both their interface and target patterns can match one concrete
pair. Inherent declarations conflict only when overlapping target patterns export the same method
name or both export destruction.

Destruction is intentionally uniform across a nominal family. An instance containing `drop`
cannot have a predicate and must mention every target slot through one distinct binder. Allowing a
type's need for destruction to vary by refinement would make generic ownership and ABI decisions
conditional; Phase 5 rejects that model instead of approximating it. Refined method-only instances
remain ordinary patterns.

Nocter rejects overlap rather than selecting the more concrete declaration. This keeps lookup,
ownership, lowering, and editor results independent of declaration and import order.

## Consumer Rule

Parser validation, resolver signatures, type checking, conditional conformance, associated-type
normalization, drop discovery, buildability, lowering, formatter output, AST JSON, hover,
completion, navigation, references, rename, and semantic tokens consume authored binder identities
or normalized pattern results. None may infer declaration parameters from unresolved symbol-table
failures or parse normalized display text.
