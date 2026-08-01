# Allocator and Ownership

This document defines the shared design that makes v0.2.0 owning runtime values possible. Public
syntax and type rules follow the [specification](../../spec/README.md).

## Separation of Responsibilities

| Layer | Responsibility |
|---|---|
| type checker | initialized, moved, and borrowed state of source places |
| aggregate drop shape | recursive per-type drop structure, independent of execution paths |
| runtime drop obligation | subtrees and prefixes actually owned on a path |
| IR | explicit activation, publication, transfer, and drop of obligations |
| backend | execution of IR state transitions according to ABI layout |
| `std/mem` | allocation layout, growth, deallocation, allocator provenance |
| `std/string`, `std/vec` | type-specific buffer and initialized-length invariants |

Do not collapse source-place state and runtime cleanup state into one bitset or ad hoc flag. The
former rejects invalid source operations; the latter drops only resources acquired on a failure
path.

## Allocator Contract

`Layout` is a validated allocation request, not merely a size/alignment pair.

- Alignment is nonzero, a power of two, and within the target limit.
- `count * element_size` and growth calculations use checked arithmetic before allocation.
- Zero-sized values and zero-capacity collections use a canonical empty state with no allocation.
  Free distinguishes the presence of an allocation.
- `RawBuffer` retains its pointer, allocated byte length, alignment, and allocator provenance.
- Free uses the allocator and actual layout from allocation time, never logical length as allocation
  size.
- Grow proceeds as allocate → transfer initialized bytes/elements → publish new state → free old
  allocation. Failure before publication leaves the old state unchanged.
- Public callers cannot construct or mutate the `RawBuffer` representation.

OS syscalls stay inside allocator implementations. `String` and `Vec<T>` use a private `RawBuffer`
and do not call page primitives directly.

## Recursive Drop Model

A runtime obligation represents at least these states recursively:

```text
Inactive
Complete(shape)
ArrayPrefix { completed, element_shape }
StructFields { initialized fields }
PayloadFields { active variant, initialized fields }
```

Constructing a fixed-array element does not add it immediately to the completed array prefix:

1. Create an independent recursive obligation for the current element.
2. Initialize fields or payloads one at a time and publish them to that obligation.
3. After the whole element completes, move the current obligation into the array and increment the
   completed prefix.
4. On intermediate failure, recursively drop the current element, then drop the completed prefix in
   reverse order.

This represents nested partial initialization that a completed-element count alone cannot express.

## Collection Ownership

`Vec<T>` maintains these invariants:

- `0 <= len <= capacity`
- the allocation provides capacity elements of storage, but only `[0, len)` is owned
- every element in `[0, len)` is complete; `[len, capacity)` is uninitialized
- drop and clear destroy `[0, len)` exactly once in reverse order
- reserve transfers element ownership to new storage without duplicating elements
- push increments `len` only after publishing a complete destination-element obligation
- pop transfers the last element to tracked return storage and decrements vector `len`

Even when non-copy `T` storage moves through raw bytes, the semantic operation is relocation rather
than copying. Invalidate obligations in the old storage before freeing it so drop glue cannot run
twice.

`String` is a specialized buffer that owns a UTF-8 byte prefix. Bytes have no drop behavior, but
`String` shares `0 <= len <= capacity`, failure-atomic growth, and allocator provenance with
`Vec<T>`.

## Error-path Invariants

- Register every value acquired by an initializer in an obligation before the next fallible
  operation.
- Publish a value as complete only after all subvalues initialize.
- Complete a replacement value under a separate obligation before destroying and replacing the old
  value.
- Clean up in reverse acquisition order without replacing the original error.
- Represent ownership transfer as one IR-level transition that invalidates the source obligation and
  activates the destination obligation.

## Rejected Shortcuts

- implementing `Vec<T>.clear` as only `len = 0`
- duplicating non-copy element bytes while both copies remain live
- freeing by logical length instead of allocation size
- leaving a half-updated state with an old pointer and new capacity after failure
- letting the backend inspect AST to guess what to drop
- implementing arbitrary-position removal as an exception to the prefix model

If arbitrary-position removal becomes necessary, design sparse ownership state as a separate model.
