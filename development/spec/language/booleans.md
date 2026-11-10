# Boolean Operations

## Scope

This chapter owns the adopted v2.0.0 intrinsic bool negation and short-circuit operations. It does
not complete the whole block/control grammar or introduce user-defined truth conversion.
[Comparisons](comparisons.md) own equality and ordering; the [grammar](syntactic-grammar.md#expressions-places-and-statement-boundaries)
owns the operators' recognition and precedence.

A bool value is true or false. `!value` requires bool and returns its negation. Both operands of
`left && right` and `left || right` require bool; their result is bool. No integer, optional,
resource or other data type acquires implicit truthiness.

## Evaluation and Completion

Evaluate the left operand first. For &&, false is the result without evaluating the right operand;
otherwise evaluate and return the right operand. For ||, true is the result without evaluating the
right operand; otherwise evaluate and return the right operand. Operand evaluation retains ordinary
calls, updates, transfers, failures, interruption and cleanup. A skipped operand performs none of
those operations, but remains subject to structural name/type and const-operation obligations.

Normal completion is separate from a known result. `check() || true` returns true whenever it
completes normally; this does not prove that check returns, suppress its effects or authorize
omitting the call. If the selected right operand cannot complete normally, the logical expression
cannot complete through that path. A possible short-circuit path can still produce its result.
A break in a skipped operand cannot establish a continuing loop exit.

## Proof and Optimization

Checking may derive a Boolean result shared by every possible normal completion and use that fact
for control, initialization and access proofs. Negation, logical composition and value-preserving
boundaries need no repeated source annotation. Unknown storage values or callable results cannot
be assumed constant without their own evidence; aliasing and updates must not invalidate a retained
fact. No proof may manufacture a value for an interrupted operation.

Knowing a result does not erase its evaluation. A selected branch may become unconditional only
after the condition's required evaluation has been preserved. In particular, an unused known
result cannot suppress a required arithmetic failure or discard a completed update. Required
constant evaluation consumes the same selected operations and short-circuit control.
