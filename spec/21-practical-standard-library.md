# Practical Standard Library

This chapter records the practical standard-library contract. These APIs are ordinary Nocter
source declarations distributed with the compiler. Only target boundaries such as opening a file
descriptor are compiler-owned primitives.

## Text and Collections

`std/string` provides UTF-8 validation and byte-oriented search:

- `find` and `find_from` return byte offsets;
- `contains`, `starts_with`, and `ends_with` do not allocate;
- `split` rejects an empty separator and returns independently owned `String` values;
- `get_range`, `strip_prefix`, and `strip_suffix` return validated borrowed views;
- `split_views` and `lines` return allocation-free borrowed iterators;
- `String.from_utf8` validates a byte slice before copying it into owned storage.

Search and range indices are UTF-8 byte offsets. `get_range` rejects endpoints that divide an
encoding. `split_views` retains both its text and separator while iteration remains live and
matches the component boundaries of owned `split`. `lines` recognizes LF and CRLF, preserves bare
CR, and does not synthesize a final empty line. Borrowed operations preserve source provenance
through a compiler-validated typed projection; raw-pointer reconstruction is not a substitute.
Returning owned `split` components keeps their lifetime independent of the input.

Text and collection observation are type-owned. `str` declares text methods and `[T]` declares
slice methods; `String` and `Vec<T>` reuse them through one-step receiver coercion. `Vec<T>` owns
readonly and readwrite index declarations whose bodies project through its initialized slice.
Their internal raw-view helpers are not importable APIs. Explicit view construction uses `as`, for
example `(&text) as &str` and `(&values) as &[T]`.

`Vec<T>.retain` preserves relative order. Rejected elements are dropped exactly once, retained
elements move at most once, and the initialized prefix is updated only after compaction.
`Vec<T>.truncate` drops the removed suffix.

`str` defines equality once, and `String` reaches it through readonly coercion. `[T]` defines
element-wise equality, `contains`, and `position` under `where (&T == &T): bool`; `Vec<T>` reaches the same
implementation through its readonly slice coercion.

`str` also defines bytewise lexical strict ordering, and `[T]` defines lexicographic ordering under
`where (&T < &T): bool`. `String` and `Vec<T>` reach those declarations through the same readonly
coercions; neither nominal container duplicates the algorithms.

The iterator terminal operations `find`, `contains`, `position`, `any`, `all`, `fold`, and `to_vec`
remain default methods of `Iterator`. Their item type is `Self.Item`; they use static generic
dispatch and do not require runtime interface objects. `contains` and `position` consume the
iterator, borrow each yielded owner for equality, and destroy every yielded owner exactly once,
including the item that causes early return.

## Formatting

`std/fmt.Format` is the static contract used by owned string interpolation:

```nct
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
```

The distributed library conforms `str`, `String`, `bool`, and every built-in integer. A nominal
project type may conform and implement its representation with the public `append_*` functions.
The `try_append_*` variants expose recoverable allocation to explicit builders; `format_into` and
interpolation use the ordinary aborting allocation policy. Formatting dispatch is static and does
not require a runtime interface object.

## Paths and Files

`std/path.Utf8Path` is an owned, NUL-free UTF-8 path. The name is intentional: operating systems
may support filename bytes that are not UTF-8, and this type does not claim to represent them.
`join` replaces the base when its child is absolute and otherwise inserts one path separator. It
does not perform filesystem normalization or canonicalization.

`std/io.File` can open an existing file for reading, create or truncate a file for writing, and
open a file for append. The `open_path`, `create_path`, and `append_path` functions accept
`Utf8Path`. File handles close once when explicitly closed or dropped.

`Reader` and `Writer` define the shared byte-I/O contracts. `Reader.read` initializes no more than
the supplied buffer length and returns zero at end of stream. The `read_to_end` default method
collects bytes into independently owned `Vec<u8>` storage. A reader that reports an impossible byte
count fails with `std.io.invalid_read_count`. The `read_to_string` default method uses the same
collector and validates the complete result as UTF-8 before returning an independently owned
`String`.

`Writer.write_text` is a default adapter from UTF-8 text to the complete-byte `write` contract.
`BufReader` and `BufWriter` in `std/io/buffer` own their buffering storage and receive these common
operations through static interface dispatch. A buffered writer reports I/O failure only through
an explicit `flush` or `close`; dropping it discards unflushed bytes because destruction cannot
return an error. Successful flush clears the buffer only after the underlying write succeeds.

## Numeric Text and Process State

`std/num` parses decimal `usize`, `i32`, and `u8` values without allocation. Invalid syntax and
overflow return `none`. Formatting functions return owned decimal `String` values; paired `try_*`
functions use an explicit recoverable allocator.

`std/process.arg_count`, `arg`, `environment_count`, and `environment` query process-lifetime
storage without allocating. Out-of-range indexed queries return `none`; invalid process encoding
returns `error`. `args` remains the allocating convenience that collects all arguments.

## Allocation and Failure

Normal `String`, `Vec`, path, split, buffered-I/O, and formatting construction uses the current
allocation context and aborts if allocation cannot continue. `String` conversion and numeric
formatting also expose explicit `try_*` operations for a recoverable `TryAllocator`. Filesystem,
I/O, invalid UTF-8, invalid paths, and empty split separators remain recoverable `T!` failures.
