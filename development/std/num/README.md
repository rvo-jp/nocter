# Integer Text

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

There are no type-named free-function aliases. This contract does not add floating-point values,
arbitrary radix parsing, locale rules, or a matrix of public integer-to-integer conversions.
