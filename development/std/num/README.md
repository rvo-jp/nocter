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
