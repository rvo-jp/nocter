# Universally Unique Identifiers

The compiler-checked [`std/uuid` contract](index.nct) owns UUID byte layout and canonical text.
`Uuid` is a copyable value containing exactly 16 bytes. Parsing accepts exactly 36 lowercase ASCII
characters with hyphens in the `8-4-4-4-12` positions; formatting always returns that spelling.
Equality, ordering, hashing, byte observation, parsing, and formatting derive from the same stored
bytes.

`Uuid.v4()` fills all 16 bytes through `std/random`, then sets the RFC 9562 version-4 and RFC 4122
variant bits. It is allocation-free and returns `std.random.unavailable` if the operating-system
entropy source fails. The supported Darwin entropy provider is a bounded system call and is not a
`blocking` operation in Nocter's effect model.

Random UUIDs provide practical opaque identifiers. They are not secret tokens, authentication
credentials, signatures, or proof that two independently supplied identifiers are globally unique.
