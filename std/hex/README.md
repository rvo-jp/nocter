# Hexadecimal Bytes

The compiler-checked [`std/hex` contract](index.nct) owns canonical hexadecimal conversion for
complete byte sequences. Encoding always emits two lowercase ASCII digits per byte. Decoding is
strict: uppercase letters, non-ASCII digits, and incomplete byte pairs are rejected rather than
normalized implicitly.

`encode_into` and `decode_into` perform no allocation. They validate the complete required size and
all input syntax before the first destination mutation, so failure leaves caller-owned output
unchanged. They write only the returned prefix and leave later output bytes untouched.

`encode` and `decode` provide owned `String` and `Vec<u8>` results under the ordinary allocation
policy. Both surfaces consume the same digit and validation decisions; the owned forms do not
define a second hexadecimal dialect.
