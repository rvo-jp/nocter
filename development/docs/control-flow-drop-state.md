# Path-Sensitive Aggregate Cleanup

This document owns the compiler architecture for runtime aggregate cleanup state introduced by
v0.11.0 Phase 7. Public move, drop, and assignment rules remain in the
[ownership specification](../../spec/05-ownership-borrowing-drop.md); this document defines how accepted programs
reach native IR without duplicating ownership analysis.

## Responsibility Boundary

Type checking and ownership analysis decide whether a source operation is legal on every reachable
path. IR lowering does not recover that proof and does not turn an invalid use into a runtime
check. Its narrower responsibility is to emit destruction exactly when a value is live on the path
that reaches cleanup.

A purely static `DropObligation` is sufficient for straight-line code. It is insufficient after a
branch whose paths do not perform the same whole-value operation: cloned lowering contexts can
describe each branch, but neither clone is the runtime state after the join. The aggregate local is
therefore promoted to a runtime live flag only when lowering encounters a path-sensitive move,
drop, or reinitialization.

## State Model

An aggregate local retains its slot, layout, copy capability, destructor plan, and static partial
initialization obligation. It may also retain one `BoolLocation` with this meaning:

| Flag | Runtime meaning |
|---|---|
| `true` | the local owns a complete value that requires its configured cleanup |
| `false` | the local has been moved, destroyed, or has not completed initialization |

Promotion allocates the flag once and initializes it from the current static obligation before
control flow can diverge. Repeated promotion returns the same location and emits no second
initialization. Aggregates that are copyable, have no destructor plan, or remain straight-line do
not need the flag.

Partial initialization retains its existing field, payload, or prefix drop state. If a partially
initialized construction can cross a new control-flow boundary, the outer live state and the
existing fine-grained state compose; neither replaces the other.

## Transition Placement

Runtime transitions are part of the operation that changes ownership:

- successful complete initialization or reinitialization emits `SetBool(true)` after the value is
  established
- a whole-value move emits `SetBool(false)` after the value has been transferred and before later
  cleanup can run
- explicit destruction emits `SetBool(false)` after destructor and field cleanup complete
- failed construction leaves the flag false and uses the existing partial-initialization cleanup
- copy operations do not change the source flag

Expression lowering must place transitions where evaluation happens. In particular, a move in the
right operand of `&&` or `||` belongs inside that operand's conditional instructions. Appending one
unconditional transition after the whole condition would incorrectly mark an unevaluated value as
moved.

## Cleanup

`PendingAggregateDrop` is the single boundary between local state and cleanup lowering. When it
carries a live flag, the generated complete or partial drop program is wrapped in an IR condition
on that flag. Normal scope exits, returns, propagated failures, `break`, `continue`, match cleanup,
and replacement assignment all consume this same representation.

Syntax-specific lowering may request promotion or emit an ownership transition. It must not build
its own guarded destructor sequence. The independent `DestructSignature` remains the sole source
of destructor identity and generic substitution.

## Control-Flow Integration

Before lowering a non-terminal control-flow construct, the compiler discovers outer aggregate
locals whose state may change in its conditions or reachable blocks and promotes those locals in
the parent context. Branch clones therefore reference the same flag location. Their static
obligations can still diverge for local cleanup decisions; the parent does not guess a merged
compile-time state.

Loops execute the same transitions on each real iteration. `break`, `continue`, and returns append
cleanup through their established scope marks. Whether the source may read a value on a later
iteration remains an ownership-analysis question.

## Invariants

- one aggregate slot has at most one runtime live flag
- promotion precedes every path that can mutate the flag
- a transfer clears the source only after the transfer has consumed its bytes
- initialization sets the destination only after the complete value exists
- cleanup reads the flag through `PendingAggregateDrop`; it never infers liveness from branch shape
- branch cloning never allocates competing flags for the same outer local
- native buildability does not reject a construct that ownership and lowering both represent
- semantic declaration and type identities, never source spelling or standard-library names,
  determine aggregate and destructor behavior

## Verification

Focused IR tests must assert promotion, transition order, short-circuit placement, guarded cleanup,
and the absence of flags in straight-line code. Native tests must use observable destructors to
cover taken and untaken `if`/`match` paths, loop zero/one/multiple execution, move arguments,
bindings, assignments, explicit drop, reinitialization, early exits, and imported alias types.

The complete phase gate is the repository verification script, documentation generation, and a
clean diff check.
