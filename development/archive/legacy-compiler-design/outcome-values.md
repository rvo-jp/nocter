# First-Class Outcome Values

This document owns the compiler design for v0.3.0 Phase 6 stored optional and fallible values.
Public value, ownership, and control-flow semantics belong to the specification. The completed
milestone gate belongs to the [v0.3.0 Release Record](../../releases/v0.3.0.md).

## Completion Record

v0.3.0 Phase 6 completed on 2026-08-03. The compiler implements the recursive stored
layout, explicit callable bridges, active-payload ownership and cleanup, saved-value consumers,
aggregate and fixed-array storage, generic specialization, provenance retention, and normalized
analysis/LSP presentation described here. Repository-home and packaged-home native tests cover the
adopted outcome shapes. Repeated equal layers and deeper recursive shapes remain outside the
completed boundary.

## Boundary

Phase 5 made `T?`, `T!`, and one optional/fallible composition executable at callable return
boundaries. It deliberately required immediate consumption. Phase 6 removes that boundary: every
supported outcome shape becomes an ordinary sized value that may be bound, moved, assigned, passed,
returned, and stored inside another sized value.

The supported recursive shapes remain exactly those adopted by Phase 5:

```text
T
Optional(T)
Fallible(T, error)
Fallible(Optional(T), error)
Optional(Fallible(T, error))
```

Repeated equal layers and deeper recursion remain rejected by the shared shape capability.

## Shared Storage Layout

`OutcomeShape` owns both callable-channel order and stored layout construction. A stored layer uses
one machine-word tag followed by aligned union storage:

```text
Optional(T)       = tag + union { T, empty }
Fallible(T)       = tag + union { T, error }
Fallible(T?)      = tag + union { Optional(T), error }
```

Tag zero denotes presence or success. Tag one denotes absence or failure. Only the active branch is
initialized. Nested tag and payload offsets are derived structurally; IR, frame layout, backend,
drop lowering, and analysis never infer them from source spelling.

The callable ABI remains register-oriented and may place tags and payload words differently from a
memory object. Explicit pack/unpack operations bridge the two representations. This prevents frame
layout details from becoming a public ABI promise.

## Semantic IR Identity

IR preserves the semantic identity of every supported outcome layer. A single optional return is
`Optional(T)`, a single fallible return is `Fallible(T)`, and a two-layer return records its ordered
outer and inner layers. Optional and fallible calls may share an ABI shape, but they do not share a
type variant.

Shared call instructions and payload operations use outcome terminology. Error payload capture,
`catch`, and fallible failure returns remain fallible-only operations. Backend validation rejects a
normal call to any outcome return, an optional call paired with an error-only failure mode, and a
return instruction whose layer disagrees with its function return type. Lowering therefore cannot
recover a lost layer by re-reading source syntax or reconstructing a typechecker decision.

Contextual generic inference treats postfix consumption and implicit outcome construction as
separate operations. For `return next(value)?` in a function returning `T?`, `next` is specialized
from the payload context `T`; the collector must not infer `T?` and create `(T?)?`. Stored payload
consumption uses the same destination-based operation for scalars, views, and borrows, so a borrow
payload cannot fall back to a call-only lowering path.

## Value Operations

- A binding initializes its tag and active payload before the value becomes live.
- Moving an outcome transfers the active branch and its drop obligation exactly once.
- Copying is permitted only when every possible payload is copyable.
- Assignment evaluates the replacement first, drops the old active payload, and then publishes the
  replacement tag.
- Aggregate and fixed-array storage recursively use the same outcome layout and initialization
  rules.
- Outcome parameters use ordinary value passing derived from the stored value layout; call sites do
  not require immediate unwrapping.

## Consumers

`?`, `!`, `otherwise`, and `catch` consume an arbitrary outcome-producing place or expression. A
call result is no longer a special case. The consumer reads exactly one semantic layer, transfers
the selected payload, and leaves inactive storage untouched.

Value-producing catch lowering is owned by
[Catch Recovery Lowering](value-producing-catch.md). Stored outcomes expose their layer, payload,
and error offsets to that common consumer; they do not own a separate terminal-only catch path.

Cleanup remains part of the ordinary scope-drop plan. Branch lowering may inspect tags, but it must
not maintain a parallel ownership system.

## Provenance

Stored outcomes retain provenance by semantic channel. Presence or success carries its payload
provenance; absence is storage-independent; failure carries error provenance. Binding, moving,
aggregate storage, and later consumption preserve this distinction. Joining control flow may widen
origins but may not merge absence with failure or invent an initialized payload.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| normalized layer tree and stored offsets | `outcomes/storage` |
| source typing and contextual `none`/`error` construction | `typecheck` |
| tag-aware move, assignment, and drop state | `typecheck/ownership` |
| semantic IR outcome identity and call operations | `ir/model` and `ir/lower/types` |
| frame slots and stored value operations | `ir/lower/outcome_values` |
| callable ABI pack/unpack | `ir/lower/expressions/calls` and `backend/codegen/outcomes` |
| outcome return encoding and layer validation | `backend/codegen/outcome_returns` and `backend/codegen/validation` |
| active-payload memory operations | `backend/codegen/outcome_values` |
| channel-specific storage origins | `typecheck/provenance` |
| hover, completion, and signature presentation | `analysis`; protocol conversion in `driver/lsp` |

New responsibilities use new modules. Buildability may reject only shapes outside the adopted
capability; it must not enumerate expression spellings that happen to be implemented.

## Verification

Tests must observe every tag, payload, ownership, and storage path for scalar/view, direct aggregate,
indirect aggregate, and move-only payloads. Required cases include binding and later consumption,
copy and move, replacement, parameter and return round trips, aggregate/fixed-array nesting,
absence/failure distinction, cleanup on all exits, provenance across storage, generic
specialization, JSON-RPC presentation, and packaged native execution.
