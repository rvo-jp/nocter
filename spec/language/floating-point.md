# Floating-Point Values

Nocter provides `f32` and `f64` as copy value types with IEEE 754 binary32 and binary64 storage.
Every bit pattern is a valid stored value. Neither type owns memory, carries storage provenance, nor
requires destruction.

## Literal Typing

The accepted token grammar is defined in
[Floating-Point Literals](lexical-grammar.md#floating-point-literals). A suffix fixes the literal's
type. An unsuffixed literal uses an expected `f32` or `f64` type and otherwise defaults to `f64`.
Another expected type or a conflicting suffix is an error.

Conversion from the exact authored decimal value uses round-to-nearest, ties-to-even. The compiler
does not parse through its host language's floating-point type. A finite literal that rounds to
infinity and a nonzero literal that rounds to zero are errors. Representable subnormal literals are
valid. Unary `-` is a separate expression operator.

```nct
let default_width = 1.5       // f64
let narrow: f32 = 1.5        // f32 from expected type
let explicit = 1.5f32        // f32 from suffix
let tiny = 1.0e-45f32        // representable subnormal
```

## Arithmetic and Comparison

Primitive `+`, `-`, `*`, `/`, `%`, and unary `-` require floating operands of one matching type and
return that type. There is no implicit integer/float conversion or implicit `f32`/`f64` widening.
Arithmetic uses IEEE round-to-nearest, ties-to-even. Division by zero, overflow, underflow,
infinity, and NaN do not trap and do not produce a fallible result. `%` uses a quotient truncated
toward zero.

`==`, `!=`, `<`, `<=`, `>`, and `>=` use IEEE comparison. Positive and negative zero compare equal.
NaN compares unequal to every value, including itself, and every ordering comparison involving NaN
is false. Inclusive comparisons follow the shared derived-comparison rule and therefore require
both `<` and `==`.

Arithmetic may quiet a signaling NaN. The sign and payload of a NaN produced by arithmetic are
target-defined. Code that requires a stable representation must use the bit conversion API.

## Representation API

`std/num` owns exact bit conversion and allocation-free classification:

```nct
construct f32 { pub noalloc func from_bits(bits: u32): Self }
construct f64 { pub noalloc func from_bits(bits: u64): Self }

instance f32 {
    pub noalloc method self.to_bits(): u32
    pub noalloc method self.is_nan(): bool
    pub noalloc method self.is_infinite(): bool
    pub noalloc method self.is_finite(): bool
    pub noalloc method self.is_zero(): bool
    pub noalloc method self.is_normal(): bool
    pub noalloc method self.is_subnormal(): bool
    pub noalloc method self.is_sign_negative(): bool
    pub noalloc method self.is_sign_positive(): bool
    pub noalloc method self.abs(): Self
    pub noalloc method self.floor(): Self
    pub noalloc method self.ceil(): Self
    pub noalloc method self.trunc(): Self
    pub noalloc method self.round_ties_even(): Self
}
```

`f64` provides the same methods with `u64` bits. `from_bits` and `to_bits` preserve all bits,
including signed zero and NaN payloads. The sign methods inspect the representation and therefore
also classify zero and NaN. `abs` clears only the sign bit.

`floor`, `ceil`, and `trunc` round toward negative infinity, positive infinity, and zero.
`round_ties_even` selects the nearest integral value and chooses the even neighbor at an exact tie.
Rounding preserves an already integral value, infinity, NaN classification, and signed zero.

The module constants `F32_INFINITY`, `F32_NEG_INFINITY`, `F32_NAN`, `F64_INFINITY`,
`F64_NEG_INFINITY`, and `F64_NAN` provide special values. Their module subject remains explicit;
they are not prelude globals.

`total_compare` returns `std/order.Ordering` and orders every representation. Negative NaNs precede
negative infinity; negative zero precedes positive zero; positive infinity precedes positive NaNs.
NaN sign, signaling bit, and payload participate in the order. This named operation does not make
ordinary floating comparison total and does not give either floating type a `TotalOrder`
implementation.

## Explicit Conversions

`as` accepts only numeric conversions that preserve every value of the source type. `f32 as f64`
is lossless. An integer type may convert with `as` only when all its values are exactly
representable in the destination floating type. Floating-to-integer conversion, `f64 as f32`, and
other rounding or narrowing conversions require named library operations rather than inheriting a
machine instruction's sentinel or saturation behavior.

Every built-in integer declares `from_f64(value: f64): Self?`. It succeeds only when `value` is
finite, integral, and inside the destination type's mathematical range. It accepts either sign of
zero as integer zero and rejects NaN, infinity, fractional values, and out-of-range values. A
binary32 input can first use the lossless `as f64` conversion; the checked decision is still made
once by the destination constructor.

`f32.from_f64(value)` rounds a finite value to nearest with ties to even. It returns `none` when a
finite input would become infinity or when a finite nonzero input would become zero. Finite rounded
results, both infinities, both zero signs, and NaN classification are otherwise retained. This
range-checked narrowing does not promise to preserve a NaN payload.

Contextual literal typing is not conversion. Write `1.0` for a floating-point value rather than
expecting the integer token `1` to change domains.

## Target Consistency

The selected target owns decimal rounding and compile-time floating arithmetic. Checked constants
retain the resulting `f32` or `f64` bits through lowering. Runtime arithmetic uses the target's
matching operations. Compile-time and runtime results therefore do not depend on the compiler
host's floating-point parser.

## Decimal Parsing

The standard constructors `f32.parse(text)` and `f64.parse(text)` consume one complete
locale-independent spelling. Decimal input has this form:

```text
"-"? digits ("." digits)? (("e" | "E") ("+" | "-")? digits)?
```

At least one integer digit is required. A written decimal point requires a following digit.
Whitespace, separators, a leading `+`, and trailing characters are invalid. The exact spellings
`inf`, `-inf`, and `NaN` are also accepted so canonical formatted non-finite values can round trip.

The complete decimal significand and exponent denote an exact rational value. Parsing rounds that
value once to the destination format using round-to-nearest, ties-to-even. A nonzero finite value
that would become zero and a finite value that would become infinity return `none`; representable
subnormals are accepted. Negative decimal zero preserves its sign. Parsing may use temporary
storage in the current allocation context, but invalid text and numeric range loss are reported by
the optional result rather than by an error payload.

## Decimal Formatting

`f32.to_string()` and `f64.to_string()` produce the shortest locale-independent decimal spelling
that parses back to the same type and exact IEEE representation. `try_to_string(allocator)`
produces the same spelling with recoverable destination allocation. Both methods and the
`std/fmt.Format` implementations use one generation authority; interpolation therefore cannot
choose a different decimal representation.

Finite values use fixed notation when the exponent of the first significant decimal digit is from
-6 through 20. Other finite values use a lowercase `e`, with an explicit `+` for a nonnegative
exponent. The significand has no redundant trailing zeroes or decimal point. Positive zero is `0`,
negative zero is `-0`, infinities are `inf` and `-inf`, and every NaN is `NaN`. NaN formatting is a
value spelling rather than a payload-preserving serialization; use `to_bits` when the payload is
observable data.

Decimal generation uses exact integer interval arithmetic and round-to-nearest, ties-to-even
boundaries. Its fixed workspace is sized from the binary64 representation limit and does not
allocate. Recoverable formatting therefore reports only destination `String` growth failure.
