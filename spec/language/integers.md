# Integers and Numeric Operations

This chapter defines integer literal typing, explicit integer conversion, fixed-width arithmetic,
and arithmetic traps.

## Integer Literals

Integer literal rules:

- Accepted integer literal syntax is defined in [Lexical Grammar](lexical-grammar.md#integer-literals).
- An unsigned N-bit type has the inclusive range `0` through `2^N - 1`. A signed N-bit type has
  the inclusive range `-2^(N - 1)` through `2^(N - 1) - 1` and uses two's-complement
  representation. On the current target, `usize` is `u64`-width and `isize` is `i64`-width.
- Integer literals start as untyped integer literals.
- If an integer literal has an expected integer type, it takes that type when the value fits.
- When unary `-` applies directly to an integer literal, with any number of grouping parentheses
  between them, range checking uses the combined negative mathematical value. For an expected
  signed N-bit type, the positive magnitude may therefore be at most `2^(N - 1)`, allowing the
  exact minimum value. An expected unsigned type is invalid because unary `-` requires a signed
  operand.
- Without an expected type, a unary-negative integer literal is checked as `i32`, including the
  `-2^31` minimum case. A larger magnitude does not cause inference of a wider type.
- The signed-minimum case becomes one checked integer constant. It does not first construct an
  out-of-range positive value and does not execute a runtime negation overflow.
- If no context fixes the type, the literal becomes `i32`.
- Assigning an out-of-range literal is a type error.
- Non-literal integer values are not implicitly converted between integer types.

Examples:

```nct
let a = 10        // i32
let b: u64 = 10   // u64
let c: u8 = 300   // error: literal out of range
let d = 0xFF_FF   // i32
let e: u8 = 0b1010
let minimum: i8 = -128
let too_small: i8 = -129 // error

let x: i32 = 10
let y: u64 = x    // error: no implicit integer conversion
```

Floating-point literals and the deliberately narrow integer/floating conversion boundary are
defined separately in [Floating-Point Values](floating-point.md).

## Numeric Operations and Conversions

Numeric operations do not perform implicit integer conversion.

Rules:

- Integer binary arithmetic uses operands of the same integer type.
- Shift operators use operands of the same integer type. Integer literals on the right side may be contextually typed by the left operand type when the value fits.
- Shift expressions return the left operand type.
- Left shift moves the fixed-width bit pattern toward the most-significant end, discards bits that
  leave the width, and fills low bits with zero. Discarded bits do not cause an arithmetic overflow
  trap.
- Unsigned right shift fills high bits with zero. Signed right shift uses two's-complement
  arithmetic shift and fills high bits with the original sign bit.
- A zero shift count leaves the value unchanged. A negative count or a count greater than or equal
  to the left operand's bit width traps before any machine shift is executed.
- Signed division truncates the mathematical quotient toward zero. Signed remainder satisfies
  `a = (a / b) * b + (a % b)`, has absolute value less than the absolute value of `b`, and is zero
  or has the same sign as dividend `a`. Unsigned division and remainder use the ordinary
  non-negative quotient and remainder.
- For a signed type, both `minimum / -1` and `minimum % -1` trap as division-family overflow even
  though the mathematical remainder would be zero.
- Integer literals may take an expected integer type when the value fits.
- Non-literal integer values are not implicitly converted.
- `bool` does not implicitly convert to or from integer types.
- Explicit integer conversion uses `expr as Type`.
- Integer `as` is allowed only for lossless conversions.
- Narrowing integer conversions are not allowed with `as`.
- Signedness-changing integer conversions are not allowed with `as` unless the target type can represent every value of the source type.
- On the ARM64 macOS target, `usize` has the same range as `u64`, and `isize` has the same range as `i64` for conversion checking.

The same `as` expression can explicitly select a one-step type-owned borrow coercion when the
source is already borrowed. That distinct contract is specified in
[Borrow Coercions](borrow-coercions.md); it does not relax the numeric rules above.

Examples:

```nct
let a: u32 = 10
let b: u64 = 20

let c = a + b          // error
let d = (a as u64) + b // OK
```

```nct
let x: u32 = 10
let y: u64 = x as u64 // OK: lossless widening

let signed: i32 = 10
let unsigned = signed as u64 // error: not lossless for all i32 values

let big: u64 = 300
let small = big as u8       // error: narrowing
let checked = u8.from_u64(big)   // u8?
let truncated = u8.truncate(big) // u8
```

`from_u64` and `truncate` are explicit numeric conversion APIs declared as construction functions by
the active standard-library package. Their names have no special grammar meaning:

```nct
construct u8 {
    pub noalloc func from_u64(value: u64): Self?
    pub noalloc func truncate(value: u64): Self
}
```

Explicit `u64` bit-mixing APIs are ordinary methods:

```nct
let sum = left.wrapping_add(right)
let difference = left.wrapping_sub(right)
let product = left.wrapping_mul(right)
let product_high = left.multiply_high(right)
let common = left.bit_and(right)
let combined = left.bit_or(right)
let mixed = left.bit_xor(right)
let rotated = left.rotate_right(amount)
let leading = left.leading_zeros()
```

The wrapping operations compute modulo 2^64. `multiply_high` returns bits 64 through 127 of the
full unsigned product. The bit methods apply fixed-width operations, and `leading_zeros` returns 64
for zero. `rotate_right` uses the low six bits of `amount`, so every `u64` amount is valid. These
methods do not allocate or fail. Normal arithmetic operators retain the trapping rules below; the
methods do not introduce new operator spellings.

Arithmetic trap rules:

- Overflow in normal integer arithmetic traps.
- Wrapping arithmetic must use an explicit wrapping API.
- Division by zero traps.
- Remainder by zero traps.
- Signed division-family overflow traps for both `/` and `%` when the operands are the minimum
  signed value and `-1`.
- Shift counts greater than or equal to the bit width of the shifted value trap.
- Shift counts must be non-negative.
- Left-shift bit loss is not integer overflow. It follows the fixed-width bit-shift rule and does
  not trap.

Trap semantics are specified in [Control Flow](control-flow.md#never-and-reachability). These arithmetic safety checks are always-on for every build mode; see [Safety Checks and Build Modes](control-flow.md#safety-checks-and-build-modes).
