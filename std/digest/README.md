# Cryptographic Digests

The compiler-checked [`std/digest` contract](index.nct) currently provides SHA-256 as an explicit
algorithm. `Sha256` owns incremental computation state and accepts any sequence of byte chunks.
`Sha256Digest` is a copyable value that owns exactly 32 bytes; completing a state cannot retain or
borrow message storage.

Digest text is exactly 64 lowercase hexadecimal digits. Parsing rejects uppercase, prefixes,
whitespace, incomplete values, and every noncanonical spelling. Formatting, hashing, equality,
ordering, byte observation, and parsing all derive from the same stored byte sequence.

SHA-256 detects content changes and supports interoperable content identity. It does not provide
authentication, password hashing, encryption, signatures, key storage, or a constant-time
execution guarantee.
