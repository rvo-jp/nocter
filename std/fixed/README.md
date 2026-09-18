# Fixed-Capacity Collections

The compiler-checked [`fixed` module contract](index.nct) is the sole authority for public
signatures. This guide records the representation and failure guarantees that span those
declarations.

`FixedVec<T, N>` stores up to `N` owned values directly inside the value. It requests no backing
allocation and retains no allocator. Empty slots use optional storage so every slot remains
initialized even when `T` is move-only. `try_push` returns the unchanged input value when full;
capacity failure therefore neither loses ownership nor creates an `error`. Owning iteration yields
initialized values in insertion order and drops any unconsumed values normally. An operation such
as `clear` that destroys `T` does not promise `noalloc`, because that promise also covers the
element destructor rather than only the collection's storage policy.

`ByteBuffer<N>` uses dense inline bytes for protocol and I/O staging. `spare_mut` exposes initialized
but uncommitted capacity, and `commit` advances the observable prefix only after the caller has
written it. A failed commit leaves the prefix unchanged. Its slice coercion and iterator expose
only committed bytes.

Fixed-width scalar appends delegate byte order to [`std/bytes`](../bytes/README.md). They check the
complete required capacity before committing anything. Failure returns `false` with the committed
prefix unchanged; success commits exactly the scalar width.

Variable-width scalar appends use the same spare-prefix transaction. The byte codec first proves
that the complete canonical representation fits and returns its width; `ByteBuffer` commits that
width only afterward. Insufficient capacity returns `none` and leaves the committed prefix
unchanged.

Use these types when an explicit upper bound is part of the application contract or allocation is
forbidden. Use [`Vec`](../vec/README.md) when input size is not naturally bounded or retaining a
large maximum inline capacity would make every value unnecessarily large.
