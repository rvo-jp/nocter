# Owned Strings

The compiler-checked [`String` module contract](index.nct) is the sole authority for exact public
declarations. This chapter defines the longer invariants of owned UTF-8 storage without repeating
those signatures.

`String` owns initialized UTF-8 bytes and exposes borrowed observation through its readonly
coercion to `str`. An original `String` member wins before receiver coercion; general search,
comparison, slicing, trimming, and borrowed iteration are therefore defined once by
[`str`](../str/README.md).

Normal copying, reservation, and append operations use the current aborting allocator. Their
`try_` counterparts use a `TryAllocator` and report recoverable storage failure. Byte append first
validates the complete input and publishes no bytes when validation or allocation fails. Scalar
append uses the shared UTF-8 encoder. No public operation may leave invalid UTF-8 in the initialized
prefix.

`empty` defers allocator selection until first growth. `with_capacity`, including a zero-capacity
request, records the allocator selected by construction so later growth cannot silently move to a
different allocation context. Capacity is measured in bytes and never changes logical text length
by itself.

Scalar-safe suffix mutation, Unicode case conversion, and their exact failure rules are defined in
[Unicode Text and Scalars](../char/README.md). Raw internal buffer operations are implementation
details and are not importable String APIs.
