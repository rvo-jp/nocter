# Pointer and Address Conversion

The compiler-checked [`std/ptr` contract](index.nct) is the sole authority for exact public pointer
conversion declarations. Raw-pointer type semantics and the absence of general dereference belong
to [Strings, Arrays, Views, and Pointers](../../../spec/language/sequences-and-text.md#raw-pointers-and-address-api).

The public operations convert an existing raw pointer to its numeric address or derive a non-owning
raw pointer from a readonly or readwrite borrow. They preserve no ownership and grant no new read,
write, dereference, or lifetime authority. A pointer derived from a borrow may remain as an address
value after the borrow ends, but the language does not guarantee that the address still identifies
valid storage.

For a zero-sized pointee, borrow conversion returns a non-null address satisfying the pointee's
alignment. Distinct logical places or fixed-array elements may have equal numeric addresses, so
address equality is not value or place identity for zero-sized types.

Address-to-pointer conversion and construction of raw views are package-internal trusted
operations. They are not importable user APIs merely because the public module exposes conversions
in the opposite direction.
