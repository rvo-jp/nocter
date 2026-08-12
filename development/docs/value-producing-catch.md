# Catch Recovery Lowering

The public behavior of `catch` belongs to
[Errors and Optional Values](../../spec/04-errors-optionals.md). This document owns the compiler
boundary that joins a fallible success payload with a reachable fallback result.

## Semantic Boundary

Type checking removes exactly one fallible layer from the operand and checks a reachable fallback
result against that success type with the common block-result assignability service. The catch
environment adds a named `error` local only for the named binding form. Return checking,
initialization state, ownership, allocation analysis, and expected-expression facts traverse the
same block with that environment; none of them infer recovery from source spelling.

The success and fallback branches are mutually exclusive initializers. Borrow provenance joins the
operand's success provenance with the fallback result provenance. Catch-local owned storage may not
escape, while a borrow of the caught error's `code` or `message` retains the error channel's
provenance.

## Destination-Driven Lowering

The surrounding consumer selects the result destination before lowering the failure handler. The
same destination is passed to scalar, borrow, view, direct-aggregate, indirect-aggregate, field,
argument, assignment, return, and stored-outcome lowering. A recovering handler initializes that
destination and rejoins; a terminating handler emits its explicit control flow. The operand is
evaluated once in both cases.

`LoweredFallbackResult` distinguishes a result that can continue from a result that deliberately
enters another terminal outcome handler. The shared block lowering then places catch-local cleanup
on the correct side of that result. This is especially important for `T?!`: a catch fallback of
`none` enters the existing optional-absence handler, while a present payload initializes the final
destination and skips that handler.

The IR `OutcomeFailureMode::Catch` carries error destinations, handler instructions, and whether a
rejoining path exists. Backend call emitters use that semantic flag to branch past success-register
storage after recovery. They never reconstruct recovery from instruction contents or copy a stale
success register over the fallback destination.

## Ownership and Cleanup

Fallback expressions move their value into the selected destination before remaining catch-local
values are dropped in reverse initialization order. A moved fallback local no longer owns a drop
obligation. A terminal fallback uses the ordinary function or loop exit cleanup. On success, no
error local or fallback local becomes live; on failure, no uninitialized success payload is
dropped.

Stored outcomes and immediate calls use the same failure-mode builder. Composed outcomes preserve
layer order: catch consumes only the fallible layer, and a following `otherwise` consumes only the
optional layer. Optional calls, stored optional values, present payload promotion, and `none` all
route through one optional-result handler.

## Editor Boundary

The AST and syntax are unchanged. Hover, semantic tokens, definition, references, rename,
completion, and visible-local collection traverse the catch block as an ordinary block. Named
bindings retain one local identity across the fallback result; discarded bindings create no
identity. Expected-type facts for the block result come from the fallible success type, so editor
analysis does not need a catch-specific textual fallback.

## Verification

Unit coverage owns type compatibility, branch state, provenance, exact IR modes, cleanup, and
composed-layer routing. Native tests cover success, recovery, terminal handling, scalar and
aggregate destinations, stored values, optional presence and absence, caught-error fields, and
exactly-once execution. Source-corpus, public-example, distributed-home, and framed-LSP tests retain
only their integration boundaries.
