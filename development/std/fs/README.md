# Filesystem

`std/path.Utf8Path` is an owned, NUL-free UTF-8 path. The name is intentional: operating systems
may support filename bytes that are not UTF-8, and this type does not claim to represent them.
`join` replaces the base when its child is absolute and otherwise inserts one path separator. It
does not perform filesystem normalization or canonicalization.

The path owns its complete spelling while the queries in the compiler-checked
[`std/path` contract](../path/index.nct) return allocation-free views into that spelling.

These queries are byte-lexical over the ASCII `/` separator and never access the filesystem.
Because `/` and `.` are single-byte UTF-8 characters, every returned boundary is a valid UTF-8
boundary. Trailing separators are ignored before selecting the final component. An empty spelling
or a spelling containing only separators has no file name, stem, extension, or parent. `.` and `..`
remain ordinary lexical component spellings; the queries do not resolve them.

`file_name` returns the last nonempty component. `extension` returns the nonempty suffix after the
last `.` in that component only when the dot is neither its first nor last byte. A leading dot by
itself therefore does not create an extension, and a trailing dot has no extension. `file_stem`
returns the file name with that recognized extension and its preceding dot removed, or the complete
file name when no extension is recognized.

`parent` removes the final nonempty component and adjacent separators while preserving an authored
leading root-separator run. It otherwise returns the exact remaining prefix. A single relative
component and a spelling made only of root separators have no parent. Repeated separators, `.`
components, and `..` components are not otherwise normalized.

`std/io.File.open`, `File.create`, and `File.append` respectively open an existing file for
reading, create or truncate a file for writing, and open a file for append without blocking the
executor. Their returned computation must be awaited. `Utf8Path` coerces to `&str`, so the same
constructors accept a borrowed path without parallel `_path` functions. `BlockingFile` provides
the same operations synchronously. Both owner types close once when explicitly closed or dropped;
later operations on an explicitly closed value fail with `std.io.closed`.

The canonical `read`, `read_to_string`, `write`, `write_text`, `metadata`, `exists`, `remove_file`,
`rename`, `create_dir`, `create_dir_all`, `remove_dir`, and `read_dir` functions are asynchronous. Whole-file transfer
composes `File` with the executor-safe byte interfaces; path mutation transfers complete owned path
bytes to the bounded file service. Metadata queries transfer the path and publish portable facts
rather than a target `stat` record. Directory acquisition and record batches use the same bounded
service and descriptor-retirement authority. Their `_blocking` twins use the explicit synchronous
surface. Recursive traversal and removal remain open until their executor-safe contracts are
complete. Pure `Metadata` and `DirEntry` inspection remains unqualified.

The target syscall boundary returns raw `{ value, errno }` facts. `std/io` retries interrupted open,
read, and write operations, completes partial writes before reporting success, rejects a
zero-progress write, and maps other errno values into the public built-in `error`. Close is issued
once and is not retried because an interrupted close may already have consumed the descriptor.
Borrowed string paths are checked for NUL before a target call.

`std/fs` provides the path-oriented one-shot operations and owning directory stream declared in
its compiler-checked [public contract](index.nct).

`Metadata` is a snapshot containing portable entry classification, byte length, and the
target-reported last-content-modification instant. `modified` returns that instant as
`std/time.SystemTime`; callers use the single time module authority for UTC conversion and RFC 3339
generation. Darwin metadata nanoseconds are validated before publication. An invalid target value
fails metadata construction with `std.fs.invalid_metadata_time` rather than escaping as a malformed
`SystemTime`.

`read` and `read_to_string` asynchronously open an existing entry and return independently owned
storage. `read_to_string` validates the complete file as UTF-8 and preserves the ordinary
`std.string.invalid_utf8` failure when validation fails. `write` and `write_text` create or
truncate the destination and return success only after the complete input has passed through the
`Writer` contract. These four functions compose `File`, `Reader`, and `Writer`; they do not define
a second descriptor-I/O algorithm. `read_blocking`, `read_to_string_blocking`, `write_blocking`,
and `write_text_blocking` provide the same whole-file policy through `BlockingFile`,
`BlockingReader`, and `BlockingWriter`.

`metadata` follows symbolic links. `metadata_blocking` is its synchronous twin.
`symlink_metadata` and `symlink_metadata_blocking` instead inspect the final path entry itself, so
they report `FileType.symlink` for a symbolic link, including a dangling link. Intermediate links
are still followed. A metadata value's `len` is the target-reported byte length represented as
`u64`; it is not a collection index and therefore is not narrowed to `usize`. `regular` and
`directory` have their ordinary target meanings. Sockets, devices, and every other entry kind are
reported as `other`. `is_file` and `is_directory` are exact tests of that portable classification.

`read_dir` asynchronously opens exactly one directory and returns an owning `ReadDir`.
`ReadDir.next` asynchronously returns `DirEntry?!`: the optional layer distinguishes clean end of
stream and the failure layer reports an error encountered after construction. `read_dir_blocking`
and `BlockingReadDir.next_blocking` expose the same policy synchronously. Neither stream implements
`Iterator`, because the current iterator contract has no recoverable per-step failure channel.
Entry order is the target's directory order and is not sorted. `.` and `..` are never returned.

The asynchronous worker reads each raw record batch into job-owned storage. Only completion copies
the initialized prefix into the stream buffer, so a running or abandoned worker never retains a
pointer into caller-owned mutable storage. Both surfaces then use the same record decoder, UTF-8
policy, path joining, entry classification, and malformed-record validation.

Each entry owns its UTF-8 file name and the path formed by joining the opened path spelling and
entry name, independently of the stream buffer. The joined path is not made absolute or
canonical. A name that is not valid UTF-8 fails with `std.fs.invalid_utf8_name`; it is never
skipped or lossily converted. `DirEntry.file_type` classifies the entry itself without following a
symbolic link. Symbolic links therefore return `FileType.symlink`; an unknown or nonportable target
kind returns `other`. The type is a directory-entry snapshot and callers must perform a later
filesystem query when races matter.

End of stream, explicit `close`, a step failure, and destruction each converge on the same
close-once state. Asynchronous close passes through the file-service retirement authority;
blocking close directly owns its descriptor transition. After any terminal event, the respective
next operation returns `none`. An interrupted target read is retried before it becomes a public
failure. A malformed target record fails with `std.fs.invalid_directory_record`, closes the stream,
and cannot be retried against the same buffer. Opening a non-directory fails with
`std.io.not_directory`.

`exists` returns `false` only when the target classifies the path as absent, including a missing
component or a dangling symbolic link. Permission denial and every other failure remain errors.
The absent path does not require construction of a built-in error value. This makes `exists` a
convenience query rather than a mechanism for hiding access failures. `exists_blocking` provides
the same policy synchronously.

`remove_file` removes one non-directory entry. When the path names a symbolic link, the link itself
is removed rather than its target. `rename` performs one target rename operation; on the current
target it replaces an existing destination when the OS permits. It does not fall back to copying
and deleting across filesystems. The canonical forms execute through owned blocking jobs;
`remove_file_blocking` and `rename_blocking` invoke the same target behavior synchronously. All
filesystem functions accept `Utf8Path` through its existing readonly coercion and reject an
embedded NUL before any OS operation.

`create_dir` creates exactly one directory with target-default permissions filtered by the process
umask. It fails with `std.io.already_exists` when the final spelling already names any entry.
`create_dir_blocking` is its synchronous twin.
`create_dir_all` asynchronously walks the authored spelling from left to right and creates every
missing directory prefix. `create_dir_all_blocking` applies the same prefix policy synchronously.
They succeed when a prefix already resolves to a directory, including through a symbolic link, but
fail when an existing prefix is not a directory. An empty spelling is invalid input; a spelling
containing only root separators succeeds without a mutation. The operations do not lexically
resolve `.`, `..`, or repeated separators before passing prefixes to the target. They are not
transactional: directories created before a later prefix failure remain present. Both surfaces use
one pure prefix-boundary scanner; target execution is the only divergent responsibility.

`remove_dir` removes exactly one empty directory. It is never recursive and never follows a final
symbolic link as a directory. A nonempty directory fails with `std.io.directory_not_empty`.
`remove_dir_blocking` is its synchronous twin.
`remove_file` and `remove_dir` remain distinct so source states whether it intends to remove a
non-directory entry or an empty directory. Both asynchronous workers and blocking implementations
attempt a mutating target call once: they do not blindly retry after an interruption whose
completion state could be ambiguous.

Errno classification, syscall numbers, and metadata layout are dependency-free,
target-specific `std/internal/os` responsibilities. The allocator-backed temporary path argument
is a separate package-internal path responsibility shared by `std/process`, `std/io`, and `std/fs`;
this keeps the raw OS fact layer independent from memory allocation policy. Mapping an OS
classification into the stable built-in `std.io.*` family is a package-internal I/O policy shared
by `File` and `std/fs`, not an OS ABI responsibility. None of these internal contracts is exposed
by `std/fs`; an operation may add reporting context without changing the root code.
