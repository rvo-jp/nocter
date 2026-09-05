# Vectors

The compiler-checked [`Vec` module contract](index.nct) is the sole authority for exact public
declarations. This chapter defines the longer invariants of contiguous owning storage without
repeating those signatures.

`Vec<T>` owns an initialized sequence and exposes readonly and readwrite observation through its
coercions to `[T]`. Slice lookup, comparison, indexing, and ordering are implemented once by
[`slice`](../slice/README.md); Vec does not provide forwarding algorithms for that surface.

`retain` preserves relative order. Rejected elements are dropped exactly once, retained elements
move at most once, and the initialized prefix advances only after each successful compaction step.
`truncate` drops every removed suffix element exactly once. Ownership transfer operations do not
copy move-only elements.

Zero-sized element types use logical length and capacity without allocating backing bytes.
Reservation, insertion, removal, iteration, and destruction still perform their normal ownership
effects once per logical element.

`empty` defers allocator selection until first growth. `with_capacity`, including a zero-capacity
request, records the allocator selected by construction so later growth cannot silently move to a
different allocation context. Recoverable constructors and mutations use `TryAllocator`; ordinary
growth follows the aborting policy defined by [Allocation and Failure](../mem/README.md).
