# Checked Integer Operations

## Scope

This chapter owns the adopted v2.0.0 intrinsic integer data and arithmetic contract. It does not
introduce implicit conversions, bitwise operations or user operator families.
[Comparisons](comparisons.md) own equality and ordering; the
[grammar](syntactic-grammar.md#expressions-places-and-statement-boundaries) owns precedence.

The fixed-width signed types are `i8`, `i16`, `i32` and `i64`; the corresponding unsigned types are
`u8`, `u16`, `u32` and `u64`. For width N, a signed value ranges from -2^(N-1) through 2^(N-1)-1;
an unsigned value ranges from zero through 2^N-1. Signed storage uses two's complement.
`isize` and `usize` use the selected target's pointer width. Every named integer type has its own
data identity, even when its width and range equal another type's. All integer values are Copy;
owning a value does not grant update access to its source.

Unary negation and binary addition,
subtraction and multiplication produce their exact mathematical result when representable in the
selected integer type.
An unrepresentable result is a safety failure; there is no implicit wrapping, saturation or wider
result type. Operand preparation follows the ordinary value model: an earlier scalar operand is
captured before evaluation of a later operand that can update its source.

## Pointer-size Integers and Literal Context

The currently qualified 64-bit target profile gives `usize` values from 0 through
18446744073709551615 and `isize` values from -9223372036854775808 through 9223372036854775807.
Compiler host integer
size does not select this range. Another profile requires corresponding evaluator and runtime
representation qualification before admission.

Addition, subtraction and multiplication require their mathematical result to fit the selected
integer data type. Unsigned subtraction below zero fails; it does not wrap. Negating an unsigned
value has the same checked meaning as subtracting it from zero of that type. Only zero succeeds.
A negative integer literal cannot supply an unsigned type, including in dead source, unless its
value is zero. Direct unary minus on an integer literal, including grouping, is range-checked
together with its magnitude, so a signed type's minimum value is expressible without an invalid
positive intermediate.

An integer literal is checked with the integer data required by a selected binding, input, result,
aggregate component or arithmetic context. Without a determining context it defaults to i32.
A tree of these arithmetic operations whose leaves are integer literals can receive that context
as a whole, including grouped expressions and unary negation. Within one arithmetic/comparison
operation, an already typed integer operand can determine the other operand's literal expression,
on either side. Two literal-only
operands without another integer context use i32. A named constant, place or callable result
retains its declared/inferred data type; its numerical value does not make it a contextual literal.

This is literal typing, not implicit integer conversion. Arithmetic and primitive comparison
require equal operand data types. Context cannot reinterpret a typed integer as another integer type,
change a selected callable, or insert a conversion. Runtime operand evaluation and snapshots
remain in source order even when constraints are obtained from the other operand.

## Division and Remainder

Division truncates the mathematical quotient toward zero. Its result must be representable in the
result type. Division by zero therefore fails. Dividing any signed type's minimum value by -1
also fails.
Unsigned division computes the nonnegative integral quotient; its remainder is between zero and
the divisor minus one.

Remainder directly computes the mathematical remainder associated with truncating division. For a
nonzero divisor, `a = b * q + r` holds in mathematical integers, and r is zero or has a's sign.
Only r is the operation's result; q need not fit in the result type. Consequently every signed
type's minimum value modulo -1 is zero.
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
