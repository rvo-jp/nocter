# Numeric Values

## Floating-Point Representation

The built-in `f32` and `f64` types expose their exact IEEE 754 representations through
`from_bits` and `to_bits`. These operations preserve every bit, including the sign of zero and a
NaN payload. Classification methods distinguish zero, subnormal, normal, infinity, and NaN values
without allocating. `is_sign_negative` and `is_sign_positive` inspect the representation, so they
also classify signed zero and NaN. `abs` clears only the sign bit.

`F32_INFINITY`, `F32_NEG_INFINITY`, `F32_NAN`, `F64_INFINITY`, `F64_NEG_INFINITY`, and `F64_NAN`
provide module-scoped special values. Arithmetic may canonicalize a NaN; use `from_bits` when an
exact payload is part of a protocol or file format.

`floor`, `ceil`, and `trunc` round toward negative infinity, positive infinity, and zero.
`round_ties_even` rounds to the nearest integer and selects the even neighbor at an exact tie.
These operations preserve an already integral value, infinity, NaN classification, and the sign of
zero. They do not allocate and return the same floating-point type.

## Checked Numeric Conversion

Every integer type exposes `from_f64`. It accepts only finite, mathematically integral values in
the destination range; fractional values, infinities, NaNs, and overflow return `none`. A binary32
caller widens with `as f64` without information loss before using the same conversion authority.

`f32.from_f64` performs round-to-nearest, ties-to-even narrowing. It returns `none` if a finite
value overflows to infinity or a finite nonzero value underflows to zero. Non-finite input retains
its classification. The source implementation proves these preconditions before invoking the
closed target conversion primitive, so no target saturation or sentinel value becomes public API.

The `u64` fixed-width surface supplies wrapping addition, subtraction, and multiplication; the
high half of a full unsigned product; bit conjunction, disjunction, and exclusive-or; rotation;
and leading-zero count. These operations provide the exact wide arithmetic required by decimal
conversion algorithms without introducing `u128` as a source type or asking ordinary trapping
operators to acquire a second meaning.

## Floating-Point Text Input

`f32.parse` and `f64.parse` consume an entire ASCII decimal spelling. They accept an optional
leading `-`, one or more integer digits, an optional decimal point followed by one or more digits,
and an optional `e` or `E` exponent whose own sign may be written. They also accept the canonical
non-finite spellings `inf`, `-inf`, and `NaN`. Whitespace, digit separators, a leading `+`, a
missing digit, and trailing input return `none`.

The parser retains the decimal significand as an arbitrary-precision integer and rounds the exact
rational value directly to the destination's IEEE representation with ties to even. It returns
`none` when a nonzero finite input rounds to zero or a finite input rounds to infinity. Temporary
big-integer storage uses the current allocation context; allocation failure follows the ordinary
aborting policy. Neither the compiler host parser nor target floating arithmetic defines the
result.

## Floating-Point Text Output

`to_string` emits the shortest decimal text that parses back to the same type and IEEE bits.
`try_to_string` emits exactly the same text using a supplied recoverable allocator. Fixed notation
is used for decimal exponents from -6 through 20; other values use lowercase scientific notation
with an explicit sign for a nonnegative exponent. Signed zero is preserved as `0` or `-0`.
Infinities use `inf` and `-inf`; every NaN uses `NaN` because decimal formatting does not serialize
NaN payloads.

The generation algorithm compares exact rounding intervals in a fixed-capacity, allocation-free
workspace sized for binary64. Type-owned methods, `Format`, and interpolation share this authority.
Only the destination `String` can fail to grow on a recoverable formatting path.

## Integer Text

Every built-in integer owns the decimal text surface declared by the compiler-checked
[`std/num` contract](index.nct). `parse` is
allocation-free and consumes the complete input. Unsigned types accept one or more ASCII digits.
Signed types additionally accept exactly one leading `-`. Empty input, a leading `+`, whitespace,
non-ASCII digits, another character, and a mathematical value outside the destination range return
`none`. Leading zeroes are valid, and negative zero produces zero.

`to_string` produces the shortest ordinary base-ten spelling, with `0` as the sole zero spelling
and a leading `-` only for a negative signed value. It uses the current allocation context and
aborts on allocation failure. `try_to_string` uses the supplied recoverable allocator and returns
its allocation failure. These operations and `Format` must use one decimal-generation authority;
parsing must scan an input once through one signed or unsigned decimal authority.

There are no type-named free-function aliases. This contract does not add arbitrary radix parsing,
locale rules, or a matrix of public integer-to-integer conversions.
