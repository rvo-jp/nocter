# Practical Standard Library

This chapter records the practical standard-library contract. These APIs are ordinary Nocter
source declarations distributed with the compiler. Only target boundaries such as opening a file
descriptor are compiler-owned primitives.

## Text and Collections

`std/string` provides owned UTF-8 storage and validation; the source-owned `str` instance presents
byte-oriented search and borrowed projection:

- `find` and `find_from` return byte offsets;
- `contains`, `starts_with`, and `ends_with` do not allocate;
- `split` rejects an empty separator and returns independently owned `String` values;
- `get_range`, `strip_prefix`, and `strip_suffix` return validated borrowed views;
- `split_views` and `lines` return allocation-free borrowed iterators;
- `String.concat(...parts: &str)` joins zero or more borrowed views into owned storage through the
  language argument-pack contract;
- `String.from_utf8` validates a byte slice before copying it into owned storage.

Search and range indices are UTF-8 byte offsets. `get_range` rejects endpoints that divide an
encoding. `split_views` retains both its text and separator while iteration remains live and
matches the component boundaries of owned `split`. `lines` recognizes LF and CRLF, preserves bare
CR, and does not synthesize a final empty line. Borrowed operations preserve source provenance
through a compiler-validated typed projection; raw-pointer reconstruction is not a substitute.
Returning owned `split` components keeps their lifetime independent of the input.

Text and collection observation are type-owned. `str` declares text methods and `[T]` declares
slice methods; `String` and `Vec<T>` reuse them through one-step receiver coercion. Slice indexing
is therefore the sole implementation used by direct `Vec<T>` indexing. Internal raw-view helpers
are not importable APIs. Explicit view construction uses `as`, for
example `(&text) as &str` and `(&values) as &[T]`.

`Vec<T>.retain` preserves relative order. Rejected elements are dropped exactly once, retained
elements move at most once, and the initialized prefix is updated only after compaction.
`Vec<T>.truncate` drops the removed suffix.

`Vec<T>` supports zero-sized element types. Its length and capacity remain logical element counts;
reserving, inserting, removing, iterating, and dropping such elements do not allocate backing
bytes and still perform the ordinary ownership operations once per logical element.

`String.empty()` and `Vec<T>.empty()` defer allocator selection until their first growth.
`with_capacity`, including a request for zero capacity, records the allocator selected by that
construction call so later growth cannot silently move to another allocation context.

`str` defines equality once, and `String` reaches it through readonly coercion. `[T]` defines
element-wise equality, `contains`, and `position` under `where (&T == &T): bool`; `Vec<T>` reaches the same
implementation through its readonly slice coercion.

`str` also defines bytewise lexical strict ordering, and `[T]` defines lexicographic ordering under
`where (&T < &T): bool`. `String` and `Vec<T>` reach those declarations through the same readonly
coercions; neither nominal container duplicates the algorithms.

Readwrite slices define allocation-free in-place ordering:

```nct
instance [T] {
    pub method &+self.sort(): void where (&T < &T): bool
}
```

After `sort`, no later element is strictly less than an earlier element. The operation relies on
the strict total order promised by the selected `<` implementation; it does not call equality or
accept a comparator callback. It performs no allocation, uses constant auxiliary storage, and has
`O(n log n)` worst-case comparisons and moves. It may reorder equivalent elements. Comparison
borrows elements, while rearrangement transfers ownership without copying or destroying an
element. The method reports no recoverable failure and introduces no allocation or bounds trap for
a valid slice. If an authored `<` body traps, that termination remains behavior of the selected
ordering operation; `sort` does not catch or reinterpret it.

`Vec<T>` reaches the same method through its declared `&+Vec<T> as &+[T]` coercion and does not
declare a forwarding method. Empty, one-element, already ordered, reverse-ordered, duplicate, and
move-only element sequences follow the same contract.

The allocation-free iterator terminal operations `find`, `contains`, `position`, `any`, `all`, and
`fold` remain default methods of `Iterator`. Their item type is `Self.Item`; they use static generic
dispatch and do not require runtime interface objects. `contains` and `position` consume the
iterator, borrow each yielded owner for equality, and destroy every yielded owner exactly once,
including the item that causes early return.

`ExactSizeIterator` declares `Self impl Iterator` as an interface prerequisite.
`I impl ExactSizeIterator` therefore exposes `next`, `I.Item`, and iterator default methods without
repeating an `I impl Iterator` predicate. Concrete iterator types still declare separate explicit
`Iterator` and `ExactSizeIterator` implementation facts.

Collection-owning terminals live in modules that depend on both the iterator contract and the
destination collection. `std/iter/collect.to_vec` consumes any `Iterator` into a `Vec`. It is not an
`Iterator` default method because making the core iterator module depend on `std/vec` would create
a module cycle: Vec iteration already depends on `std/iter`.

The adopted v0.21.0 associative collection direction provides representation-neutral `Map<K, V>`
and `Set<T>` rather than exposing the private hash-table strategy through `HashMap` and `HashSet`
names. Its hash coherence, mapping literal, lookup, mutation, iteration, allocation, and ordering
contracts are centralized in [Associative Collections](27-associative-collections.md).

## Formatting

`std/fmt.Format` is the static contract used by owned string interpolation:

```nct
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
```

The distributed library conforms `str`, `String`, `bool`, and every built-in integer. A nominal
project type may implement the interface and build its representation with canonical members: `output.push_str`
for text and `value.format_into(output)` for nested formatted values. The `try_append_*` free
functions remain distinct because they expose recoverable allocation to explicit builders;
`format_into` and interpolation use the ordinary aborting allocation policy. Formatting dispatch
is static and does not require a runtime interface object.

## Paths and Files

`std/path.Utf8Path` is an owned, NUL-free UTF-8 path. The name is intentional: operating systems
may support filename bytes that are not UTF-8, and this type does not claim to represent them.
`join` replaces the base when its child is absolute and otherwise inserts one path separator. It
does not perform filesystem normalization or canonicalization.

`std/io.File.open`, `File.create`, and `File.append` respectively open an existing file for
reading, create or truncate a file for writing, and open a file for append. `Utf8Path` coerces to
`&str`, so the same constructors accept a borrowed path without parallel `_path` functions. File
handles close once when explicitly closed or dropped. Explicit close makes that `File` value
terminal: later read, write, and flush operations fail with `std.io.closed` instead of retaining a
descriptor word that the operating system may reuse. The `stdout` and `stderr` constructors return
non-owning wrappers. Closing one makes that wrapper terminal without closing the process-global
descriptor used by other wrappers.

The target syscall boundary returns raw `{ value, errno }` facts. `std/io` retries interrupted open,
read, and write operations, completes partial writes before reporting success, rejects a
zero-progress write, and maps other errno values into the public built-in `error`. Close is issued
once and is not retried because an interrupted close may already have consumed the descriptor.
Borrowed string paths are checked for NUL before a target call.

`std/fs` provides path-oriented one-shot operations over those stream contracts:

```nct
pub enum FileType {
    regular
    directory
    symlink
    other
}

pub copy struct Metadata
pub struct DirEntry
pub struct ReadDir

instance Metadata {
    pub method &self.file_type(): FileType
    pub method &self.len(): u64
    pub method &self.is_file(): bool
    pub method &self.is_directory(): bool
}

instance DirEntry {
    pub method &self.file_name(): &str
    pub method &self.path(): &Utf8Path
    pub method &self.file_type(): FileType
}

instance ReadDir {
    pub method &+self.next(): DirEntry?!
    pub method &+self.close(): void
}

pub func read(path: &str): Vec<u8>!
pub func read_to_string(path: &str): String!
pub func write(path: &str, value: &[u8]): void!
pub func write_text(path: &str, text: &str): void!
pub func metadata(path: &str): Metadata!
pub func read_dir(path: &str): ReadDir!
pub func exists(path: &str): bool!
pub func remove_file(path: &str): void!
pub func rename(from: &str, to: &str): void!
```

`read` and `read_to_string` open an existing entry and return independently owned storage.
`read_to_string` validates the complete file as UTF-8 and preserves the ordinary
`std.string.invalid_utf8` failure when validation fails. `write` and `write_text` create or
truncate the destination and return success only after the complete input has passed through the
`Writer` contract. These four functions compose `File`, `Reader`, and `Writer`; they do not define
a second descriptor-I/O algorithm.

`metadata` follows symbolic links. Its `len` is the target-reported byte length represented as
`u64`; it is not a collection index and therefore is not narrowed to `usize`. `regular` and
`directory` have their ordinary target meanings. Sockets, devices, and every other entry kind are
reported as `other`. `is_file` and `is_directory` are exact tests of that portable classification.

`read_dir` opens exactly one directory and returns an owning stream. `ReadDir.next` returns
`DirEntry?!`: the optional layer distinguishes clean end of stream and the failure layer reports an
error encountered after construction. This stream does not implement `Iterator`, because the
current iterator contract has no recoverable per-step failure channel. Entry order is the target's
directory order and is not sorted. `.` and `..` are never returned.

Each entry owns its UTF-8 file name and the path formed by joining the opened path spelling and
entry name, independently of the stream buffer. The joined path is not made absolute or
canonical. A name that is not valid UTF-8 fails with `std.fs.invalid_utf8_name`; it is never
skipped or lossily converted. `DirEntry.file_type` classifies the entry itself without following a
symbolic link. Symbolic links therefore return `FileType.symlink`; an unknown or nonportable target
kind returns `other`. The type is a directory-entry snapshot and callers must perform a later
filesystem query when races matter.

End of stream, explicit `close`, a step failure, and destruction each converge on the same
close-once state. After any of those terminal events, `next` returns `none`. An interrupted target
read is retried before it becomes a public failure. A malformed target record fails with
`std.fs.invalid_directory_record`, closes the stream, and cannot be retried against the same
buffer. `read_dir` on a non-directory fails with `std.io.not_directory`.

`exists` returns `false` only when the target classifies the path as absent, including a missing
component or a dangling symbolic link. Permission denial and every other failure remain errors.
The absent path does not require construction of a built-in error value. This makes `exists` a
convenience query rather than a mechanism for hiding access failures.

`remove_file` removes one non-directory entry. When the path names a symbolic link, the link itself
is removed rather than its target. `rename` performs one target rename operation; on the current
target it replaces an existing destination when the OS permits. It does not fall back to copying
and deleting across filesystems. All filesystem functions accept `Utf8Path` through its existing
readonly coercion and reject an embedded NUL before any OS operation.

Errno classification, syscall numbers, and metadata layout are dependency-free,
target-specific `std/internal/os` responsibilities. The allocator-backed temporary path argument
is a separate package-internal path responsibility shared by `std/process`, `std/io`, and `std/fs`;
this keeps the raw OS fact layer independent from memory allocation policy. Mapping an OS
classification into the stable built-in `std.io.*` family is a package-internal I/O policy shared
by `File` and `std/fs`, not an OS ABI responsibility. None of these internal contracts is exposed
by `std/fs`; an operation may add reporting context without changing the root code.

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
return an error. Successful flush clears the buffer only after the underlying write succeeds. A
failed flush makes the writer terminal because the destination may already have accepted an
unreported prefix; retrying the complete retained buffer could duplicate output. Explicit close
also makes the wrapper terminal, and later write or flush operations fail with `std.io.closed`.
A requested writer capacity of zero is normalized to one byte.

`BufReader` additionally exposes line-oriented text input:

```nct
instance BufReader {
    pub method &+self.read_line(): String?!
    pub method &+self.read_line_into(destination: &+String): bool!
    pub method &+self.close(): void
    impl Reader
}
```

`read_line` returns an owned string, `none` only when end of stream is reached before another byte,
and an error through the outer `!` layer. `read_line_into` clears `destination` before observing the
stream, writes the next line into the same allocation when its retained capacity is sufficient,
and returns `true`; it returns `false` with an empty destination for the same clean end-of-stream
condition. An empty line therefore returns an empty present `String` or `true`, not end of stream.

Line results exclude the terminating LF byte. One CR byte immediately before that LF is also
excluded; a lone CR and every other byte are retained. EOF after line bytes returns that final
unterminated line once. Repeated line reads after EOF or explicit `close` return `none` or `false`,
and byte reads through `Reader` return zero.

UTF-8 validation applies to the complete line after newline removal, so one scalar may cross any
number of partial underlying reads. Invalid input fails with `std.string.invalid_utf8`; it is not
replaced or lossily converted. The reusable destination is empty on invalid UTF-8, underlying read
failure, or recoverable allocation failure. Any such line-step failure terminates the buffered
reader because bytes may already have been consumed from its source. Later operations observe the
terminal state instead of retrying an ambiguous partial line.

The buffered reader retains its fixed read buffer and one reusable raw line buffer. It retains no
earlier completed line and never collects the complete file. Memory is bounded by the configured
read-buffer capacity plus the largest line observed and the caller's retained destination
capacity. A requested read-buffer capacity of zero is normalized to one byte so refill always
makes progress. Underlying interrupted reads retain the ordinary `File` retry behavior before a
line operation observes failure.

## Numeric Text and Process State

`std/num` parses decimal `usize`, `i32`, and `u8` values without allocation. Invalid syntax and
overflow return `none`. Formatting functions return owned decimal `String` values; paired `try_*`
functions use an explicit recoverable allocator.

`std/process.arg_count`, `arg`, `environment_count`, and `environment` query process-lifetime
storage without allocating. Out-of-range indexed queries return `none`; invalid process encoding
returns `error`. `args` remains the allocating convenience that collects all arguments.

## Future Direction: v0.23.0 Integer Text APIs

v0.23.0 will replace the type-named `std/num` free functions with one type-owned decimal surface on
every built-in integer. The intended declarations are:

```nct
construct i8 { pub func parse(text: &str): Self? }
construct i16 { pub func parse(text: &str): Self? }
construct i32 { pub func parse(text: &str): Self? }
construct i64 { pub func parse(text: &str): Self? }
construct isize { pub func parse(text: &str): Self? }
construct u8 { pub func parse(text: &str): Self? }
construct u16 { pub func parse(text: &str): Self? }
construct u32 { pub func parse(text: &str): Self? }
construct u64 { pub func parse(text: &str): Self? }
construct usize { pub func parse(text: &str): Self? }

instance i8 {
    pub method self.to_string(): String
    pub method self.try_to_string(allocator: &+TryAllocator): String! from allocator
}
```

The `i8` instance shape above applies identically to the other nine integer types. Decimal parsing
is allocation-free and consumes the complete input. Unsigned types accept one or more ASCII digits.
Signed types additionally accept exactly one leading `-`. Empty input, a leading `+`, whitespace,
non-ASCII digits, another character, and a mathematical value outside the destination range return
`none`. Leading zeroes are valid, and negative zero produces zero.

`to_string` produces the shortest ordinary base-ten spelling, with `0` as the sole zero spelling
and a leading `-` only for a negative signed value. It uses the current allocation context and
aborts on allocation failure. `try_to_string` uses the supplied recoverable allocator and returns
its allocation failure. These operations and `Format` must use one decimal-generation authority;
parsing must scan an input once through one signed or unsigned decimal authority.

The old `parse_usize`, `parse_u8`, `parse_i32`, `usize_to_string`, `u8_to_string`,
`i32_to_string`, and paired `try_*_to_string` declarations will be removed without aliases or
compatibility wrappers. v0.23.0 does not add floating-point values, arbitrary radix parsing, locale
rules, or a matrix of public integer-to-integer conversions.

## Allocation and Failure

Normal `String`, `Vec`, path, split, buffered-I/O, and formatting construction uses the current
allocation context and aborts if allocation cannot continue. `String` conversion and numeric
formatting also expose explicit `try_*` operations for a recoverable `TryAllocator`. Repeated
`String` and `Vec` growth reserves geometrically so one-at-a-time append has amortized constant
growth cost; capacity may exceed the minimum requested amount. Checked capacity arithmetic rejects
an unrepresentable size before allocation and never wraps. Filesystem, I/O, invalid UTF-8, invalid
paths, and empty split separators remain recoverable `T!` failures.
