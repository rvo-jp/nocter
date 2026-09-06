# Floating-Point Design

This document owns the cross-crate contract for adding `f32` and `f64` to Nocter. Public behavior
does not change until the corresponding specification and implementation land together. Delivery
status and qualification evidence belong to the
[`v0.38.0` milestone](../history/milestones/v0.38.0.md).

## Source Model

`f32` and `f64` are built-in copy value types with IEEE 754 binary32 and binary64 storage. Every bit
pattern is a valid stored value. Neither type carries storage provenance, owns memory, or requires
destruction.

A decimal floating-point token must contain a decimal point or an exponent. The accepted forms are:

```text
digits "." digits exponent? suffix?
digits exponent suffix?

exponent = ("e" | "E") ("+" | "-")? digits
suffix   = "f32" | "f64"
```

Digit separators follow the integer-literal rule: `_` may occur only between decimal digits. A
leading decimal point, a trailing decimal point, hexadecimal floating-point syntax, `NaN`, and
infinity spellings are not literals. Standard-library constants expose non-finite values.

An unsuffixed literal uses an expected `f32` or `f64` type when one is available and otherwise
defaults to `f64`. A suffix fixes the type and cannot conflict with an expected type. Conversion from
the exact decimal value uses round-to-nearest, ties-to-even. A finite literal that rounds to infinity,
or a nonzero literal that rounds to zero, is rejected; representable subnormal literals are valid.
Unary `-` remains a separate expression operator.

Operands of a primitive floating-point arithmetic operation have one matching floating-point type.
The operations `+`, `-`, `*`, `/`, `%`, unary `-`, `==`, `!=`, `<`, `<=`, `>`, and `>=` are available.
There is no implicit integer/float conversion and no implicit `f32`/`f64` widening. Ordinary
arithmetic follows IEEE 754 round-to-nearest, ties-to-even. Division by zero, overflow, underflow,
infinity, and NaN do not use the integer trap rules or the fallible-result type. `%` is the remainder
whose quotient is truncated toward zero.

Equality and ordering are IEEE comparisons: both signed zero values compare equal, NaN compares
unequal to every value including itself, and every ordering comparison involving NaN is false.
Arithmetic may quiet a signaling NaN. The payload and sign of a NaN produced by arithmetic are
target-defined; classification, equality, and ordering behavior are not. `from_bits` and `to_bits`
preserve an explicitly supplied stored bit pattern exactly.

## Comparison and Total Order

The presence of `operator <` proves that a comparison operation exists; it does not by itself claim
an algebraic total order. Derived comparisons use one evaluated pair of operands:

```text
left > right   = right < left
left <= right  = (left < right) || (left == right)
left >= right  = (right < left) || (left == right)
```

Consequently, `<=` and `>=` require both applicable `<` and `==` operations. This definition
preserves unordered floating-point behavior instead of incorrectly treating incomparability as less
than or equal.

The standard library owns a separate `TotalOrder` interface whose prerequisites are matching `<`
and `==` operations. Conformance promises irreflexive and transitive strict comparison, equality
consistency, and comparability of every pair. Generic algorithms that require an order, including
in-place sorting, require `T impl TotalOrder`; a structural `<` requirement alone permits only the
authored comparison. Integer and standard text types conform. Floating-point types do not conform
because their ordinary comparisons are partial.

The standard comparison module exposes an `Ordering` value and floating-point `total_compare`
methods implementing the IEEE total-order predicate. Callers choose that named operation when NaN
payloads and signed zero must participate in a deterministic total order. This does not change the
meaning of ordinary floating-point operators or silently make floats eligible for default sorting.

## Explicit Conversion Boundary

`as` remains restricted to conversions that preserve every value in the source type's complete
domain. `f32 as f64` is therefore valid. An integer type may use `as f32` or `as f64` only when every
value of that integer type is exactly representable by the destination. `f64 as f32`, floating-point
to integer conversion, and the remaining integer-to-floating conversions use named checked or
explicitly rounding standard-library operations. They never inherit a target instruction's
out-of-range sentinel or saturation behavior accidentally.

Literal contextual typing is not conversion. An integer token does not become a floating-point value
merely because a floating-point result is expected; source writes `1.0` when it means a
floating-point literal.

## Representation and ABI Closure

Stored `f32` has size and alignment 4. Stored `f64` has size and alignment 8. Aggregate layout uses
those stored facts exactly like other fields. Only a scalar whose complete concrete type is `f32` or
`f64` receives floating-point scalar transport; an aggregate containing floating-point fields keeps
the ordinary aggregate classification.

Machine owns one closed value classification with an explicit register class:

- general scalar or aggregate words use the general-purpose class;
- `f32` and `f64` scalars use the floating-point class;
- indirect values use the existing general-purpose pointer transport;
- zero-sized values consume neither class.

On `arm64-darwin`, scalar floating-point arguments use `v0` through `v7` and scalar floating-point
results use `v0`. General arguments and results retain `x0` through `x7` and `x0`/`x1`. The two
argument banks advance independently in authored argument order. Exhausting one bank spills later
values of that class to the outgoing stack without closing the other bank. Stack slots retain
authored order among spilled values and the existing alignment rules. Machine produces the complete
transport plan once; ARM64 may not infer a register class from a semantic type or opcode.

The ARM64 allocator has distinct general-purpose and floating-point virtual-register banks. Moves,
loads, stores, calls, returns, and arithmetic consume already classified machine values. Aggregate
copy, optional/fallible representation, source ownership, and provenance do not inspect
floating-point payloads.

## Constant Evaluation Authority

One target floating-point evaluator owns decimal conversion, arithmetic folding, comparison, and
bit conversion. Lexer and parser retain spelling and structure but never use host floating-point
parsing to decide a semantic value. Checking asks the evaluator for typed bits and diagnostics.
CheckedProgram, MIR, and Machine carry those bits without reparsing the token.

Runtime and compile-time evaluation must agree on finite results, infinities, signed zero,
classification, and comparison. Because arithmetic NaN payloads are target-defined, folding may
choose any target-permitted quiet NaN and must not claim payload stability beyond that contract.
Explicit `from_bits`/`to_bits` round trips remain exact and cannot be rewritten through arithmetic.

This authority is parameterized by the selected target even while `arm64-darwin` is the only
target. Adding another backend therefore cannot reuse the compiler host as an unrecorded semantic
decision.

## Pipeline Ownership

| Decision | Sole owner | Downstream contract |
|---|---|---|
| Token boundaries and retained spelling | lexer/parser | floating literal syntax node |
| Contextual type and literal diagnostic | checking | typed scalar constant |
| Decimal rounding and folded result bits | target evaluator | `f32`/`f64` bit value |
| Source operation and conversion selection | checking | selected checked operation |
| Stored size and alignment | Machine layout | closed stored layout |
| Argument/result register class and stack placement | Machine ABI planner | transport plan |
| ARM64 instruction and physical register | ARM64 lowering | encoded operation |
| Parsing, formatting, classification, and rounding APIs | standard library | declared public API |
| Hover, completion, and semantic source ranges | semantic presentation | editor projection |

No downstream phase may parse literal text again, infer float width from an instruction, recover
ABI class from a rendered type name, or implement a second host-based constant evaluator. Source
projection presents the checked literal and selected operation; it does not create editor-only
floating-point meaning.

## Standard-Library Surface

The standard library declares `primitive type f32` and `primitive type f64` beside the integer
types. Their initial practical surface includes:

- finite and non-finite classification;
- sign and signed-zero observation;
- absolute value, floor, ceiling, truncation, and nearest rounding;
- exact `to_bits` and `from_bits` operations;
- complete text parsing and shortest round-trippable text generation;
- ordinary and recoverable string allocation forms;
- checked integer conversion and checked narrowing to `f32`;
- `total_compare` returning the shared `Ordering` type;
- `Format` support through the same decimal-generation authority;
- JSON-number conversion that rejects non-finite values at the JSON boundary.

Parsing accepts one locale-independent ASCII grammar and consumes the complete input. Formatting is
locale-independent and round trips every finite value, signed zero, infinity, and NaN according to
one canonical spelling policy. The exact public declarations and observable text rules land with
their standard-library implementation rather than being duplicated here.

## Delivery Invariants

- A floating-point feature does not become public until source checking and native execution agree.
- An unsupported backend rejects floating-point use before MachineProgram construction.
- The compiler never routes ordinary float arithmetic through allocation or fallible-result
  machinery.
- Integer overflow and division traps remain unchanged.
- Default sorting cannot accept a type solely because it owns `<`.
- Formatting, interpolation, and JSON reuse the standard numeric text authorities.
- LSP features consume checked semantic products and remain available in recoverable invalid code.
- Every new representation fact has one owner and crosses each boundary in a typed contract.

## Non-goals

- decimal floating-point types
- arbitrary-precision numeric types
- complex numbers and vectors
- implicit numeric promotion
- configurable process floating-point rounding modes
- trapping ordinary floating-point arithmetic
- locale-dependent parsing or formatting
- SIMD source types or C ABI interoperability
- transcendental functions before a target-independent accuracy contract is selected
