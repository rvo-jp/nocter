# Base64 Bytes

The compiler-checked [`std/base64` contract](index.nct) owns two explicit RFC 4648 profiles.
`encode` and `decode` use the standard `+/` alphabet with required canonical `=` padding.
`encode_url` and `decode_url` use the URL-safe `-_` alphabet without padding. A decoder never
guesses the other profile, accepts mixed alphabets, ignores whitespace, or repairs padding.

Decoding rejects impossible lengths and nonzero unused bits, so every accepted spelling is the
canonical representation of exactly one byte sequence. Invalid input and insufficient output are
detected before the first caller-owned destination mutation.

The `*_into` operations allocate nothing, write only the returned prefix, and leave later output
bytes untouched. The owned operations return `String` or `Vec<u8>` under the ordinary allocation
policy while sharing the same alphabet, size, and validation decisions.
