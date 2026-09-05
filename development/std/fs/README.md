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
reading, create or truncate a file for writing, and open a file for append. `Utf8Path` coerces to
`&str`, so the same constructors accept a borrowed path without parallel `_path` functions. File
handles close once when explicitly closed or dropped. Explicit close makes that `File` value
terminal: later read, write, and flush operations fail with `std.io.closed` instead of retaining a
descriptor word that the operating system may reuse. The `stdin`, `stdout`, and `stderr`
constructors return non-owning wrappers. Closing one makes that wrapper terminal without closing
the process-global descriptor used by other wrappers.

The target syscall boundary returns raw `{ value, errno }` facts. `std/io` retries interrupted open,
read, and write operations, completes partial writes before reporting success, rejects a
zero-progress write, and maps other errno values into the public built-in `error`. Close is issued
once and is not retried because an interrupted close may already have consumed the descriptor.
Borrowed string paths are checked for NUL before a target call.

`std/fs` provides the path-oriented one-shot operations and owning directory stream declared in
its compiler-checked [public contract](index.nct).

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

`create_dir` creates exactly one directory with target-default permissions filtered by the process
umask. It fails with `std.io.already_exists` when the final spelling already names any entry.
`create_dir_all` walks the authored spelling from left to right and creates every missing directory
prefix. It succeeds when a prefix already resolves to a directory, including through a symbolic
link, but fails when an existing prefix is not a directory. An empty spelling is invalid input; a
spelling containing only root separators succeeds without a mutation. The operation does not
lexically resolve `.`, `..`, or repeated separators before passing prefixes to the target. It is
not transactional: directories created before a later prefix failure remain present.

`remove_dir` removes exactly one empty directory. It is never recursive and never follows a final
symbolic link as a directory. A nonempty directory fails with `std.io.directory_not_empty`.
`remove_file` and `remove_dir` remain distinct so source states whether it intends to remove a
non-directory entry or an empty directory. Mutating target calls are attempted once: they are not
blindly retried after an interruption whose completion state could be ambiguous.

Errno classification, syscall numbers, and metadata layout are dependency-free,
target-specific `std/internal/os` responsibilities. The allocator-backed temporary path argument
is a separate package-internal path responsibility shared by `std/process`, `std/io`, and `std/fs`;
this keeps the raw OS fact layer independent from memory allocation policy. Mapping an OS
classification into the stable built-in `std.io.*` family is a package-internal I/O policy shared
by `File` and `std/fs`, not an OS ABI responsibility. None of these internal contracts is exposed
by `std/fs`; an operation may add reporting context without changing the root code.
