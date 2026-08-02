# Nested Outcomes and Executable Process Context

This document owns the compiler and standard-library implementation design for v0.3.0 Phase 5.
Public optional, fallible, provenance, and process semantics belong to the specification. The
active completion gate belongs to the [v0.3.0 Development Contract](v0.3.0.md).

## Boundary

Nocter already typechecks optional and fallible constructors independently. The pre-Phase 5 IR
maps both `T?` and `T!` to one fallible-shaped tag. That shortcut works for one outer layer but
cannot distinguish the three leaves of `T?!`: present success, successful absence, and failure.
Buildability consequently rejects every reachable composed return and separately recognizes
`std/process.env` by declaration identity as check-only.

Phase 5 replaces that shortcut at callable boundaries. It does not promote arbitrary stored
outcome values. A composed result must be consumed immediately by propagation, fallback, catch, a
return boundary, or another supported expression lowering path.

## Compiler-Owned Outcome Shape

The compiler normalizes a resolved callable return into an ordered shape:

```text
OutcomeShape
  = Value(payload ABI)
  | Optional(OutcomeShape)
  | Fallible(OutcomeShape, error ABI)
```

Aliases and generic substitutions are resolved before construction. Buildability, IR, backend,
analysis, and LSP consume this shape; none reparses type spelling. The initial executable composed
surface contains one optional and one fallible layer in either explicit order. Deeper recursion is
rejected by shape capability, not by a standard-library name.

## Native ABI

Each layer owns one tag word and nests its success payload after that word:

```text
Fallible(Optional(T))
  x0 = fallible tag
  failure: x1..x4 = error code/message
  success:
    x1 = optional tag
    present: x2.. = T payload
    absent: no initialized T payload
```

Tag zero selects success or presence. Tag one selects failure or absence. This keeps existing
single-layer ABI behavior and makes composition structural. Indirect aggregate success storage
remains caller-provided; tags never claim uninitialized payload storage is live.

The backend must expose branch operations in terms of layer and tag, not `env`, `none`, or a source
type spelling. Return lowering writes inner payload and tag before publishing the outer success
tag. Call lowering checks outer failure before inspecting inner absence.

## Control Flow and Cleanup

- Postfix `?` removes exactly the outer layer selected by typechecking.
- `otherwise` handles exactly an optional layer and never catches an error.
- `catch` handles exactly a fallible layer and never treats absence as failure.
- A direct return may construct presence, absence, or failure according to the declared shape.
- Cleanup obligations belong to initialized payload paths. An absent path does not drop payload
  storage, and an error path does not drop an unpublished success value.
- Propagation cleanup executes before returning the corresponding tag and payload to the caller.

Outcome lowering must reuse the scope-drop and region-exit plan. It must not maintain a second list
of live owners.

## Provenance

Callable summaries retain channel-specific provenance. For `env`, the present `&str` payload is
`static`; absence carries no storage origin; the error payload follows the ordinary error summary.
For `cwd`, the successful `String` is `current` on the normal surface and derives from the explicit
allocator on the recoverable surface.

The `from` clause remains an upper-bound contract. It does not alter outcome ABI layout or decide
which branch is selected.

## Process Runtime

The low-level entry shim already captures `argc` and `argv`. Phase 5 extends the same process-
context owner with `envp` and exposes narrow target primitives for entry count and indexed entry
views. `std/process` performs name matching, separator handling, and UTF-8 validation in ordinary
Nocter source. The compiler does not provide an `env(name)` primitive.

Process-context views live for the program duration and are never freed by user code. `env` returns
successful absence when no exact name matches. An invalid requested name or invalid host entry
returns `std.process.invalid_encoding`; it must not masquerade as absence.

`cwd` keeps one recoverable implementation core. `try_cwd` receives a `TryAllocator`; normal `cwd`
uses the current aborting allocation context and adapts allocation-only failures through the
established abort path. Both surfaces preserve OS and encoding failures.

## Editor Boundary

Analysis exports the resolved nested type and provenance clause already used by hover and signature
help. Phase 5 adds channel labels only when the compiler supplies them. Completion and recovery
must remain stable inside `T?!`, an incomplete `otherwise`, and a call imported under an alias.

## Verification

Unit tests own outcome normalization, ABI classification, lowering, branch ordering, and cleanup.
CLI tests own source diagnostics. Distributed-home tests own native environment and `cwd` behavior
using only packaged compiler and standard-library inputs. JSON-RPC tests own exact normalized hover
and signature ranges.
