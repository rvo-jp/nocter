# Tuple Representation Boundary

This document defines the cross-crate representation contract for the v0.33.0 tuple milestone.
Public tuple behavior belongs exclusively to
[`spec/33-tuples.md`](../../spec/33-tuples.md). Private algorithms and module layouts remain in each
workspace crate's colocated `README.md` and Rust source.

## Design Decision

A source tuple is one structural semantic type. It is not lowered into a synthetic nominal type,
encoded as a keyed argument-pack entry, or reconstructed independently by editor and backend code.
Ordered element types are the complete identity.

The representation flows in one direction:

```text
tuple syntax
    -> semantic tuple TypeId
    -> checked positional aggregate and tuple places
    -> MIR positional aggregate and projections
    -> closed runtime tuple shape
    -> frozen MachineLayoutStore tuple layout
    -> machine aggregate reads and writes
```

Every arrow consumes the preceding contract. A later stage may attach target-specific facts, but it
must not parse source, invent element types, or repeat semantic selection.

## Sole Authorities

| Fact | Sole authority | Consumers receive |
|---|---|---|
| Parentheses denote grouping, closure syntax, callable syntax, or tuple syntax | syntax parser | immutable syntax nodes |
| Tuple identity and structural properties | semantic `TypeStore` | one interned `TypeId` and its ordered elements |
| Expression type, move/copy state, loans, and element provenance | checked-program construction | checked values, places, and binding plan |
| Runtime operations and destruction order | MIR construction | concrete positional aggregates, projections, and destruction plans |
| Concrete runtime shape | executable-program construction | closed runtime tuple element identities |
| Alignment, padding, size, offsets, and ABI class | machine-program construction through `MachineLayoutStore` | frozen layout facts |
| Registers, stack slots, addresses, and instructions | machine lowering | machine operations over frozen offsets |
| Canonical user-facing spelling | semantic presentation | rendered `(A, B, C)` text and semantic source ranges |

The source index records occurrences and semantic identities. It cannot decide whether a value is a
tuple, choose an element type, or affect checking.

## Semantic Model

The semantic type model gains a dedicated tuple kind containing at least two `TypeId` values. Type
interning derives identity from the complete ordered slice. Structural properties such as
concreteness and potential storage are calculated at interning time from those elements and then
read by consumers.

`PackEntry` remains a separate compiler-owned type. It represents keyed pack expansion and is not
source tuple syntax. Nominal structs retain declaration identity and named `FieldId` values. A
tuple element uses its position; manufacturing fake fields would incorrectly give a structural
type declaration-owned identity.

## Checked Product Contract

A checked tuple construction contains its tuple `TypeId` and checked elements in source order. A
tuple projection contains an element position and its already-resolved type. Loan and move paths
retain that position, allowing different elements to be proven disjoint without consulting syntax
or nominal declarations.

Local destructuring is represented by a recursive checked binding pattern containing local
identities, discard leaves, and tuple branches. The checked initializer operation is the sole
authority for its already-established move or copy behavior. MIR construction consumes both facts;
it does not type-check the source pattern or infer transfer behavior again.

The internal binding-pattern contract is recursive even though v0.33.0 exposes it only for local
`let` and `var` statements. Name and discard leaves plus tuple branches are sufficient. Future
parameter or control-flow patterns may consume the same checked contract only after their public
semantics are specified.

## Executable and Layout Contract

Executable-program construction rejects symbolic tuple elements just as it rejects every other
symbolic runtime type. Its runtime tuple shape contains the exact concrete element identities. It
does not create declaration or field representations for a structural tuple.

`MachineLayoutStore` produces one ordered product layout containing an element layout and offset for
each position. MIR projection, aggregate construction, and destruction all consume those positions.
Machine lowering may share generic aggregate-copy and ABI machinery with structs after layout has
erased the semantic distinction, but it must not recover tuple shape from size or source syntax.

## Presentation Contract

The semantic type renderer is the sole tuple-spelling authority. Hover, completion, inlay hints,
signature help, and diagnostics consume that renderer or its structured result. Numeric source
projections resolve through checked tuple-place evidence; they are not entered into the nominal
member index and do not receive synthetic definitions.

Formatter behavior is syntax-owned. It may choose one-line or multiline layout, but it cannot use
semantic analysis to decide whether parentheses form a tuple.

## Rejected Encodings

- **Synthetic struct per tuple type:** adds fake declarations and fields, contaminates visibility,
  navigation, conformance, and identity, and makes structural equality depend on construction
  order.
- **Reuse `PackEntry`:** conflates keyed expansion with positional products and exposes an internal
  type that semantic presentation intentionally cannot render.
- **Backend-only tuple lowering:** leaves checking without element-place identity and causes the
  backend to repeat ownership and shape decisions.
- **LSP tuple parser:** creates a second source-language interpretation and diverges under recovery.
- **Whole-tuple-only ownership:** prevents disjoint element loans and partial moves, making tuples
  materially less capable than structs without a semantic reason.

## Implementation Order

1. Add syntax nodes, semantic tuple interning, canonical presentation, and recursive local binding
   syntax. Checking must own tuple typing and produce checked element places and binding plans.
2. Carry those checked facts through MIR, destruction, runtime shape, target layout, and machine
   lowering. Execute tuple returns, calls, projections, partial moves, borrows, and drops natively.
3. Complete formatter and editor behavior, then add practical standard-library APIs whose result is
   genuinely positional. Keep named public records named.

Each step must remove temporary exhaustiveness gaps before it is committed. No stage may introduce
a compatibility representation that a later step is expected to reinterpret or delete.
