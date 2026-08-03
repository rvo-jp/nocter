# First-Class Outcome Values

This document owns the compiler design for v0.3.0 Phase 6 stored optional and fallible values.
Public value, ownership, and control-flow semantics belong to the specification. The completed
milestone gate belongs to the [v0.3.0 Development Contract](v0.3.0.md).

## Completion Record

v0.3.0 Phase 6 completed on `develop` on 2026-08-03. The compiler implements the recursive stored
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
| frame slots and stored value operations | `ir/lower/outcome_values` |
| callable ABI pack/unpack | `ir/lower/outcome_calls` and `backend/codegen/outcomes` |
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
