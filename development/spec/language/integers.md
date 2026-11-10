# Checked Integer Operations

## Scope

This chapter owns the adopted v2.0.0 intrinsic i32 arithmetic contract. It closes this fixed-width
primitive domain without importing an unspecified numeric catalog, implicit conversion or user
operator family. [Comparisons](comparisons.md) own equality and ordering; the
[grammar](syntactic-grammar.md#expressions-places-and-statement-boundaries) owns precedence.

An i32 value is an integer from -2147483648 through 2147483647. Unary negation and binary addition,
subtraction and multiplication produce their exact mathematical result when representable as i32.
An unrepresentable result is a safety failure; there is no implicit wrapping, saturation or wider
result type. Operand preparation follows the ordinary value model: an earlier scalar operand is
captured before evaluation of a later operand that can update its source.

## Division and Remainder

Division truncates the mathematical quotient toward zero. Its result must be representable in the
result type. Division by zero and -2147483648 divided by -1 therefore fail.

Remainder directly computes the mathematical remainder associated with truncating division. For a
nonzero divisor, `a = b * q + r` holds in mathematical integers, and r is zero or has a's sign.
Only r is the operation's result; q need not fit in i32. Consequently -2147483648 % -1 is zero.
Remainder by zero still fails. Compound assignment uses the same operation contract.

A target's quotient-producing instruction cannot impose an additional source failure on remainder.
A lowering must handle its instruction's exceptional inputs while preserving the selected source
contract. Equally, it cannot erase a required zero-divisor failure because the result is unused.

## Evaluation and Optimization

Evaluating an invalid checked operation at runtime safety-traps. Required constant evaluation
reports a compilation error at that operation, with no runtime fallback. Merely declaring a const
function containing a supported operation does not execute it or prove that all its inputs succeed.
Short-circuit and conditional execution retain their ordinary selected-path behavior.

Optimizations may remove a check when they establish that its failure cannot occur, or replace an
operation with an equivalent result while preserving every observable failure, update and cleanup.
Remainder has a nonzero-divisor requirement rather than a fictitious quotient-overflow requirement.
This distinction belongs to the operation contract, not to an optimizer guessing that a check was
probably unnecessary. The evaluator and native consumers preserve it explicitly.
