# Standard Library, Primitives, and OS

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## OS Error Model

Adopted: Nocter uses a layered OS error model. Target-specific raw errors are converted into common standard-library records, then into the built-in `error` payload used by `T!`.

Layering:

```text
std/os              common std plus target-gated internals: Platform, OSErrorKind, OSError, syscall, SyscallResult, Errno, errno mapping
std/error           Error, ErrorCode, and constructors for the built-in error payload
std/io              user-facing I/O APIs
std/process         user-facing process APIs
std/testing         ordinary fallible assertions for native test declarations
```

The compiler must not special-case names such as `Error`, `ErrorCode`, `OSError`, `Errno`, `File`, `args`, `env`, `cwd`, `exit`, or `abort`. These are ordinary standard-library names. The only compiler-level failure type is lowercase `error`.

`std/testing` is likewise an ordinary module. Its assertion functions return `void!` and create
stable `error` payloads through `Error.new`; assertion names and equality are not compiler syntax.

### Target Raw Errors

`std/os` owns the raw macOS syscall result and errno wrapper behind `#target: "arm64-darwin"`.

```nct
#target: "arm64-darwin"
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

#target: "arm64-darwin"
pub(nocter) copy struct Errno {
    pub code: i32
}
```

Rules:

- `SyscallResult.errno == 0` means success.
- `SyscallResult.errno != 0` means failure.
- `SyscallResult.value` is syscall-specific.
- `Errno` is a target-specific raw error wrapper.
- `Errno` is not exposed as the common OS error type.
- syscall number constants live in target-specific std internals, not in user-facing APIs.
- the exact syscall number list is a target implementation detail.

### Common OS Error

Common standard library module:

```text
std/os
```

Initial std-internal surface:

```nct
pub(nocter) enum Platform {
    macos
    linux
    windows
}

pub(nocter) enum OSErrorKind {
    interrupted
    would_block
    not_found
    permission_denied
    already_exists
    invalid_input
    broken_pipe
    timed_out
    unsupported
    unknown
}

pub(nocter) copy struct OSError {
    pub platform: Platform
    pub code: i32
    pub kind: OSErrorKind
}
```

Rules:

- `OSError` is the common OS error record.
- `OSError.platform` records the originating platform.
- `OSError.code` stores the target raw code.
- On macOS and Linux, `code` is an errno value.
- On Windows, `code` will be a Windows raw error code chosen by the Windows target design.
- `OSError.kind` is the portable classification used by higher-level standard-library modules.
- `OSError`, `OSErrorKind`, and `Platform` are `pub(nocter)` implementation details. User-facing APIs expose the built-in `error` payload instead.
- Target-gated `Errno` declarations in `std/os` are not exposed as the common OS error type.

Target-specific std internals convert raw target errors into `OSError`.

```text
SyscallResult -> Errno -> OSError
```

### Common Error Payload

The built-in failure payload is `error`. The standard library exposes ordinary names and constructors for it.

Initial public surface:

```nct
pub type ErrorCode = &str
pub type Error = error

pub func Error.new(code: ErrorCode, message: &str): Error
```

Initial standard-library implementation boundary:

```nct
pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
```

Rules:

- `error` is the compiler built-in payload type.
- `Error` and `ErrorCode` are ordinary standard-library names.
- The compiler does not know the name `ErrorCode`. `ErrorCode` is an open string classification alias over `&str`, and `Error.new` converts that string slice into the built-in `error` payload's primitive code representation.
- `new_error` is a `pub(nocter)` standard-library implementation primitive in `std/error`; user project modules must call `Error.new` instead of importing it.
- Standard-library codes use dotted names such as `"std.io.not_found"`, `"std.mem.out_of_memory"`, `"std.string.capacity_overflow"`, `"std.fmt.unsupported"`, and `"std.process.invalid_encoding"`. User and package code may use their own prefixes, such as `"app.config.missing_key"`.
- Standard-library modules should fail with `error`, not domain-specific fallible error type parameters.
- Domain-specific detail is represented by `ErrorCode`, `message`, and, where needed later, additional standard-library helper APIs.

### String and Formatting API Boundary

The signatures in the first block below are the released v0.2.0 surface. v0.3.0
Phase 0 migrates allocation-only failure to paired normal and `try_*` APIs over
the same implementation. I/O and other recoverable errors remain fallible.

Adopted: the initial owning-string and formatting boundary is split between `std/string` and `std/fmt`.

Released v0.2.0 `std/string` public surface:

```nct
pub struct String {
    storage: RawBuffer
    len: usize
}

pub func String.empty(): String
pub func String.with_capacity(allocator: &+Allocator, capacity: usize): String!
pub func String.from_str(allocator: &+Allocator, value: &str): String!
pub func String.copy(allocator: &+Allocator, value: &str): String!

pub func empty(): String
pub func with_capacity(allocator: &+Allocator, capacity: usize): String!
pub func from_str(allocator: &+Allocator, value: &str): String!
pub func view(text: &String): &str
pub func len(text: &String): usize
pub func capacity(text: &String): usize
pub func is_empty(text: &String): bool
pub func reserve(text: &+String, additional: usize): void!
pub func clear(text: &+String): void
pub func bytes(value: &str): &[u8]
pub func push_str(text: &+String, value: &str): void!
pub func capacity_overflow(): error

impl String {
    pub method &self.view(): &str
    pub method &self.len(): usize
    pub method &self.capacity(): usize
    pub method &self.is_empty(): bool
    pub method &self.bytes(): &[u8]
    pub method &+self.reserve(additional: usize): void!
    pub method &+self.clear(): void
    pub method &+self.push_str(value: &str): void!
}
```

Initial `std/fmt` public surface:

```nct
pub func append_str(out: &+String, value: &str): void
pub func append_string(out: &+String, value: &String): void
pub func append_i32(out: &+String, value: i32): void
pub func append_u8(out: &+String, value: u8): void
pub func append_usize(out: &+String, value: usize): void
pub func append_bool(out: &+String, value: bool): void

pub func try_append_str(out: &+String, value: &str): void!
pub func try_append_string(out: &+String, value: &String): void!
// `try_append_*` also covers `i32`, `u8`, `usize`, and `bool` above.
```

Rules:

- These functions are ordinary standard-library APIs, not compiler built-ins.
- `try_append_*` functions fail with the built-in `error` payload. `std/string` and `std/fmt` must
  not introduce `StringError`, `FormatError`, or another domain-specific fallible payload.
- `std/string.with_capacity` and `std/string.from_str` take an explicit `&+Allocator`.
- Every `String` stores its allocator provenance in a private `RawBuffer`. `String.empty()` binds a
  canonical zero-sized buffer to the page allocator; `with_capacity(allocator, 0)` retains the
  supplied allocator identity without performing an OS allocation.
- `reserve` grows through the bound allocator and is failure-atomic: allocation failure leaves the
  pointer, contents, length, and capacity unchanged.
- `String` storage is released by recursive deterministic drop of its private `RawBuffer`; string
  code does not call target page allocation or release primitives directly.
- The formatting append functions operate on an already-created `String`; they do not choose an
  allocator. Normal `append_*` operations adapt the shared `try_append_*` core and terminate on
  allocation failure.
- v0.3.0 interpolation lowering uses ordinary `String` and `std/fmt`
  operations with the statically propagated current allocation context. It must
  not read a mutable process-global allocator.
- `std/fmt` is not part of the initial prelude. User code imports it explicitly unless future prelude policy changes.
- Module-local helpers such as formatting `unsupported` errors are not public
  API unless explicitly listed in this chapter.

The v0.3.0 target pairs these policy surfaces:

| Aborting current-context operation | Recoverable explicit operation |
|---|---|
| `String.with_capacity(capacity): String` | `String.try_with_capacity(allocator, capacity): String!` |
| `String.copy(value): String` | `String.try_copy(allocator, value): String!` |
| `text.reserve(additional): void` | `text.try_reserve(additional): void!` |
| `text.push_str(value): void` | `text.try_push_str(value): void!` |

Both columns preserve the same buffer and publication invariants. The normal
column aborts without unwinding on allocation failure. The recoverable column
returns a stable `std.mem.*` error and leaves the old value unchanged.

### Memory and Allocator API

v0.2.0 exposes the explicit fallible surface below. v0.3.0 retains it as the
implementation baseline while introducing `TryAllocator` as the recoverable
capability and `Allocator` as its aborting adapter.

Initial public surface:

```nct
pub copy struct Layout {
    pub(nocter) size: usize
    pub(nocter) align: usize
}

pub struct RawBuffer {
    pub(nocter) ptr: *u8
    pub(nocter) len: usize
    pub(nocter) align: usize
    ...
}

pub struct Allocator {
    ...
}

pub func page_allocator(): Allocator
construct Layout {
    pub default func new(size: usize, align: usize): Self!
}
pub func layout(size: usize, align: usize): Layout!
pub func layout_size(value: &Layout): usize
pub func layout_align(value: &Layout): usize
pub func alloc(allocator: &+Allocator, size: usize, align: usize): RawBuffer!
pub func alloc_layout(allocator: &+Allocator, layout: Layout): RawBuffer!
pub func grow(allocator: &+Allocator, buffer: &+RawBuffer, new_size: usize): void!
pub func free(allocator: &+Allocator, buffer: RawBuffer): void
pub func bytes(buffer: &RawBuffer): &[u8]
pub func bytes_mut(buffer: &+RawBuffer): &+[u8]
pub func prefix(buffer: &RawBuffer, len: usize): &[u8]!
pub func prefix_mut(buffer: &+RawBuffer, len: usize): &+[u8]!
pub func out_of_memory(): error
pub func invalid_argument(): error

impl Allocator {
    pub method &+self.alloc(size: usize, align: usize): RawBuffer!
    pub method &+self.grow(buffer: &+RawBuffer, new_size: usize): void!
    pub method &+self.free(buffer: RawBuffer): void
}

impl Layout {
    pub method &self.size(): usize
    pub method &self.align(): usize
}

impl RawBuffer {
    pub method &self.bytes(): &[u8]
    pub method &+self.bytes_mut(): &+[u8]
    pub method &self.prefix(len: usize): &[u8]!
    pub method &+self.prefix_mut(len: usize): &+[u8]!
}
```

Rules:

- The released v0.2.0 operations return allocation errors. In the v0.3.0
  target, `try_*` operations retain that behavior and normal operations abort.
- The initial allocator is page-backed. General allocator families are deferred.
- Layout alignment is a non-zero supported power of two; zero-sized layouts produce canonical empty buffers without an OS allocation.
- Growth is failure-atomic and preserves the old buffer when allocation fails.
- `RawBuffer` owns raw byte storage and is not a typed collection.
- `RawBuffer`'s representation fields are `pub(nocter)`: trusted std modules
  may inspect and pass the raw storage boundary onward, but user project modules
  must obtain views through the public `bytes`, `bytes_mut`, `prefix`,
  `prefix_mut`, and method APIs.
- `RawBuffer` keeps private allocator provenance and releases owned storage during deterministic drop; `free` consumes the buffer and therefore cannot be followed by another use or release.
- User-facing collection APIs should be built on ordinary std types such as
  `Vec<T>`, not by exposing unchecked raw buffer mutation.
- Target-dependent allocation helpers are std-internal and target-gated.

### Vector API

Adopted: `std/vec` provides the initial owned variable-length collection type.
`Vec<T>` is an ordinary standard-library type, not a compiler built-in.

Released v0.2.0 public surface:

```nct
pub struct Vec<T> {
    ptr: *T
    storage: RawBuffer
    len: usize
    capacity: usize
}

pub func Vec.empty<T>(): Vec<T>
pub func Vec.with_capacity<T>(allocator: &+Allocator, requested_capacity: usize): Vec<T>!
pub func Vec.from_slice<T>(allocator: &+Allocator, values: &[T]): Vec<T>!

pub func empty<T>(): Vec<T>
pub func with_capacity<T>(allocator: &+Allocator, requested_capacity: usize): Vec<T>!
pub func from_slice<T>(allocator: &+Allocator, values: &[T]): Vec<T>!
pub func len<T>(values: &Vec<T>): usize
pub func capacity<T>(values: &Vec<T>): usize
pub func is_empty<T>(values: &Vec<T>): bool
pub func view<T>(values: &Vec<T>): &[T]
pub func view_mut<T>(values: &+Vec<T>): &+[T]
pub func reserve<T>(values: &+Vec<T>, additional: usize): void!
pub func clear<T>(values: &+Vec<T>): void
pub func pop<T>(values: &+Vec<T>): T?
pub func push<T>(values: &+Vec<T>, value: T): void!
pub func capacity_overflow(): error

impl<T> Vec<T> {
    pub method &self.len(): usize
    pub method &self.capacity(): usize
    pub method &self.is_empty(): bool
    pub method &self.view(): &[T]
    pub method &+self.view_mut(): &+[T]
    pub method &+self.reserve(additional: usize): void!
    pub method &+self.clear(): void
    pub method &+self.pop(): T?
    pub method &+self.push(value: T): void!

    drop &+self {
        ...
    }
}
```

Rules:

- `Vec<T>` is not exported by `std/prelude`; user code imports `std/vec.Vec`
  explicitly.
- `storage` is the sole allocation owner. `ptr` is a private typed alias used to form initialized
  element views and never owns or releases storage.
- Element size and alignment come from the concrete ABI layout through trusted pointer layout
  queries. Capacity multiplication is checked before allocation.
- `reserve` grows through the allocator provenance bound to `storage` and publishes the new typed
  pointer and capacity only after successful growth.
- Scalar, `&str`, fixed-array, and struct element storage paths are supported. Struct and array
  elements may own nested values and are destroyed recursively.
- `push` transfers a non-copy argument into the initialized prefix only after reserve succeeds.
  `pop` transfers the last initialized element to its return value before shrinking that prefix.
- Payload-enum element storage is deferred. Arbitrary-position insertion/removal and iterator
  helpers are absent from the released v0.2.0 surface and added by v0.3.0 Phase 2 below.
- `check` may accept `Vec<T>` APIs outside the runtime-supported element subset
  when they typecheck. `build` and `run` must reject unsupported `Vec<T>` element
  storage paths during v0.2.0 buildability validation.
- `clear` and vector drop destroy every initialized element exactly once in reverse order.
- `view` and `view_mut` expose slices over initialized elements only.

The v0.3.0 target pairs normal current-context construction and growth with
explicit recoverable operations:

| Aborting current-context operation | Recoverable explicit operation |
|---|---|
| `Vec.with_capacity<T>(capacity): Vec<T>` | `Vec.try_with_capacity<T>(allocator, capacity): Vec<T>!` |
| `Vec.from_slice<T>(values): Vec<T>` | `Vec.try_from_slice<T>(allocator, values): Vec<T>!` |
| `values.reserve(additional): void` | `values.try_reserve(additional): void!` |
| `values.push(value): void` | `values.try_push(value): void!` |

Normal and recoverable operations share layout calculation, relocation,
partial-initialization, and drop-obligation code.

v0.3.0 Phase 2 adds checked borrowed access and dense ownership-preserving mutation:

```nct
impl<T> Vec<T> {
    pub method &self.get(index: usize): &T? from self
    pub method &+self.get_mut(index: usize): &+T?
    pub method &+self.insert(index: usize, value: T): void
    pub method &+self.try_insert(index: usize, value: T): void!
    pub method &+self.remove(index: usize): T?
    pub method &self.iter(): ViewIter<T>
    pub method self.into_iter(): VecIntoIter<T>
}
```

`try_insert` validates bounds and completes capacity growth before shifting any element. Failure
leaves pointer, length, capacity, content, and storage origin unchanged. After growth succeeds,
insert and remove relocate move-only values through one transient hole with no fallible call or
externally observable edge. `ViewIter<T>` returns `&T?` tied to source storage;
`VecIntoIter<T>` owns transferred storage and returns `T?` in source order.

v0.3.0 Phase 4 adds a small shared readonly contract in `std/sequence`:

```nct
pub interface Sequence<T> {
    pub method &self.len(): usize
    pub method &self.get(index: usize): &T? from self
}

pub func first<S: Sequence<T>, T>(values: &S): &T? from values
```

`Vec<T>` explicitly implements `Sequence<T>`. `first` allocates nothing and returns an exact borrow
of the source sequence's storage. Calls through `Sequence<T>` are monomorphized to public inherent
methods; the interface has no runtime representation.

v0.3.0 Phase 7 adds ordinary iteration protocols in `std/iter`:

```nct
pub interface Iterator<T> {
    pub method &+self.next(): T?
}

pub interface Iterable<T, I> {
    pub method &self.iter(): I
}

pub interface IntoIterator<T, I> {
    pub method self.into_iter(): I
}
```

`ViewIter<T>` yields `&T`; `VecIntoIter<T>` owns transferred vector storage and yields `T`.
Collection `for` and sequence spread resolve these interfaces through explicit conformance and
static specialization. They do not recognize iterator or method names.

v0.3.0 Phase 8 adds the exact-count capability required by non-materialized literal packs:

```nct
pub interface ExactSizeIterator<T> {
    pub method &self.remaining_len(): usize
}
```

Both standard vector iterators conform to `ExactSizeIterator<T>` with their corresponding yielded
item type. `remaining_len()` reports the exact unconsumed suffix. Sequence spread calls it once
before literal-body execution and caches the checked total; iteration still terminates only through
`Iterator.next()`.

v0.3.0 Phase 9 adds allocation-transparent iterator composition. Focused modules
`std/iter/sources`, `std/iter/range`, `std/iter/chain`, `std/iter/enumerate`, and `std/iter/ops`
provide `empty`, `once`, `take`, `skip`, `chain`, `enumerate`, `count`, and `last`. Adapters own
their input iterator, yield in source order, and never create an implicit collection or runtime
interface object.
`enumerate` yields `Indexed<T>` values with public `index` and `item` fields.

Exact-size conformance is conditional. `empty` and `once` are exact unconditionally; `take`, `skip`,
and `enumerate` preserve an input's exactness; `chain` is exact only when both inputs are exact.
Iteration still stops through `next()`, and a reported exact count is never used for unchecked
element access.

`Vec.from_iter` consumes an arbitrary `Iterator<T>` through ordinary checked vector growth.
`Vec.from_exact_iter` additionally requires `ExactSizeIterator<T>` and performs one initial exact
reservation. Early exhaustion yields a shorter valid vector, while excess items continue through
checked growth. Neither API materializes an intermediate collection.

### I/O API

Adopted: `std/io` provides the initial user-facing file and text output API. The compiler must not special-case `File`, `stdout`, `stderr`, or `print`.

Initial public surface:

```nct
pub struct File {
    ...
}

pub func open(path: &str): File!
construct File {
    pub default func open(path: &str): Self!
}
pub func read(file: &+File, buffer: &+[u8]): usize!
pub func write(file: &+File, bytes: &[u8]): void!
pub func write_text(file: &+File, text: &str): void!
pub func close(file: &+File): void

impl File {
    pub method &+self.read(buffer: &+[u8]): usize!
    pub method &+self.write(bytes: &[u8]): void!
    pub method &+self.write_text(text: &str): void!
    pub method &+self.close(): void

    drop &+self {
        ...
    }
}

pub func stdout(): File
pub func stderr(): File
pub func print(text: &str): void!
```

Rules:

- `File` is a move-only standard-library type with private representation.
- `open`, `read`, `write`, and `write_text` are ordinary free-function
  wrappers around the associated function or methods.
- `File.open(path)` opens an existing file for reading in v0.2.0.
- File creation, append, truncate, read-write modes, and open options are deferred.
- `File.open(path)` maps path-related OS errors into `"std.io.not_found"`, `"std.io.permission_denied"`, or `"std.io.invalid_path"` when the target can classify them.
- `read(buffer)` reads into a writable byte view and returns the number of bytes read.
- `read(buffer)` returning `0` means end of file for regular files.
- `read(buffer)` may return fewer bytes than `buffer.len()` without treating that as an error.
- `write(bytes)` writes all bytes in the view or fails.
- Target implementations handle partial OS writes inside `write(bytes)`.
- `write_text(text)` writes the UTF-8 bytes of `&str` without encoding conversion.
- `print(text)` writes `text` to `stdout()` and does not append a newline.
- `stdout()` and `stderr()` return `File` values representing the process standard streams.
- `File` internally distinguishes owned handles from borrowed process standard streams.
- Dropping a `File` returned by `File.open(path)` closes the owned handle.
- Dropping a `File` returned by `stdout()` or `stderr()` must not close the process standard stream.
- `File.close()` closes an owned handle immediately and is idempotent. A later drop does not close
  the same handle again. It does nothing for borrowed process standard streams.
- Explicit close and the `File` drop member cannot fail. Close errors are ignored in v0.2.0.
- Unexpected OS errors are converted to `"std.os.unexpected_os_error"` with a message that preserves useful target context.

Physical placement:

- `std/io` is the user-facing module path and owns `File` plus the public I/O API.
- Target-dependent raw file-descriptor helpers live in `std/io` as `pub(nocter)` implementation details.
- Target-dependent helper functions and primitive declarations use `#target: "..."`.
- Raw helper names such as `write_text_raw` are not user-facing API.
- Common I/O API names should remain stable across targets.

Conversion flow:

```text
std/os.syscall3
    -> SyscallResult
    -> Errno
    -> std/os.OSError
    -> error
```

Not adopted in v0.2.0:

- buffered I/O
- async I/O
- seek
- directory traversal
- path object
- encoding conversion
- automatic newline printing
- compiler magic for `print`
- `println` as a compiler feature

### Process Context

Adopted: command-line arguments and environment access are standard-library APIs in `std/process`, not entry function parameters.

Released v0.2.0 public surface:

```nct
pub func args(): Vec<&str>!
pub func env(name: &str): &str?!
pub func cwd(allocator: &+Allocator): String!
pub func exit(code: i32): never
pub func abort(): never
```

v0.2.0 distribution status:

- `exit(code)` and `abort()` are runtime-shipped.
- `cwd(allocator)` is runtime-shipped on `arm64-darwin` through the standard-library syscall boundary and returns caller-owned `String` storage.
- `args()` is runtime-shipped on `arm64-darwin` and returns an owned `Vec<&str>` whose string views point into process context storage.
- `env(name)` reserves the future `&str?!` API shape, but useful runtime behavior is check-only until nested fallible/optional return lowering and process context runtime are promoted. It must not silently succeed with `none` before environment access is implemented.
- During the check-only period, `check` accepts calls to `env(name)` for
  source-level validation, while `build` and `run` reject them during v0.2.0
  buildability validation.

v0.2.0 rules:

- Entry function type parameters and value parameters, such as `func main<T>(): i32!` and `func main(args: Vec<&str>): i32!`, are not part of v0.2.0.
- The compiler must not special-case a function named `args`, `env`, `cwd`, `exit`, or `abort`.
- The generated low-level entry code may receive platform process entry information such as `argc`, `argv`, and `envp`.
- That platform information is connected to a `std/process` process context inside the active target implementation.
- User code reads command-line arguments with `std/process.args()`.
- User code reads environment values with `std/process.env(name)`.
- `args()` returns an owned `Vec<&str>` of borrowed string slices on success.
- The first argument follows the host platform convention and represents the executable path or invocation name when the platform provides one.
- The `env(name)` runtime behavior rules below are the contract for the future
  promoted implementation. During the v0.2.0 check-only period, `build` and `run`
  reject `env(name)` before lowering.
- `env(name)` has type `&str?!`.
- `env(name)` succeeds with `none` when the variable is absent.
- `env(name)` succeeds with a present `&str` when the variable is present and valid UTF-8.
- `cwd(allocator)` returns an owned current-working-directory string or fails with `error`.
- `cwd(allocator)` fails with `"std.process.cwd_failed"` if the target cannot retrieve the current working directory.
- Process context string storage is valid for the whole program.
- Argument and environment views returned by `std/process` are not owned by the caller and must be treated as borrowed view data.
- The caller must not drop process-context storage.
- The `cwd(allocator)` result is owned by the caller and must be dropped normally.
- APIs that need owned strings must explicitly copy into an allocator-owned `String`.
- Target implementations must validate argument and environment strings before exposing them as `&str`.
- The v0.2.0 `arm64-darwin` `cwd(allocator)` implementation treats the path returned by `F_GETPATH` as platform UTF-8 text; byte-to-UTF-8 validation for targets that expose arbitrary path bytes remains future std work.
- `args()` fails with `"std.process.invalid_encoding"` if any returned argument cannot be represented as UTF-8.
- `env(name)` fails with `"std.process.invalid_encoding"` if the matching environment value exists but cannot be represented as UTF-8.
- Future target implementations of `cwd(allocator)` must fail with `"std.process.invalid_encoding"` if their current-working-directory source can contain non-UTF-8 bytes and validation fails.

In the v0.3.0 target, allocating process functions use the current aborting
allocation context. `cwd(): String!` remains fallible for OS and encoding
errors, but allocation failure terminates. A recoverable-memory variant, if
needed, takes a `TryAllocator` explicitly and preserves the same OS error
identity.

Nocter v0.3.0 implements the executable process-context surface below:

```nct
pub func args(): Vec<&str>! from current | static
pub func env(name: &str): &str?! from static
pub func cwd(): String! from current
pub func try_cwd(allocator: &+TryAllocator): String! from allocator
```

v0.3.0 Phase 5 rules:

- the generated Darwin entry boundary retains `argc`, `argv`, and `envp` before entering ordinary
  Nocter code
- `args()` allocates its `Vec` in the current aborting allocation context; its `&str` elements view
  process-lifetime storage
- `args()` returns `"std.process.invalid_encoding"` if any argument is not valid UTF-8
- `env(name)` returns a process-lifetime view for an exact present name and successful `none` for an
  absent name
- an empty name, a name containing NUL or `=`, or invalid UTF-8 in a requested name or host entry
  returns `"std.process.invalid_encoding"`
- `cwd()` returns current-context-owned `String` storage; allocation failure terminates, while OS
  and encoding failures remain recoverable
- `try_cwd(allocator)` derives its result from `allocator`; allocator and OS failures remain
  recoverable and do not publish a partial result
- compiler buildability and lowering depend on resolved outcome shapes and declaration identities,
  not the spelling `std/process.env`

These v0.3.0 rules do not retroactively alter the v0.2.0 runtime contract.

Example:

```nct
use std/process.args

func main(): i32! {
    let argv = args()?

    if argv.len() < 2 {
        return 1
    }

    return 0
}
```

Physical placement:

- `std/process` is the user-facing module path.
- Process context implementation may call target-gated std internals when it depends on the target process ABI.
- Common process API names should remain stable across targets.

### Process Termination

`std/process`'s terminating functions are normal standard-library APIs.

```nct
pub func exit(code: i32): never
pub func abort(): never
```

Rules:

- `exit` is not a compiler primitive.
- `abort` is not a compiler primitive.
- `exit` does not return an error.
- `abort` does not return an error.
- `exit` terminates the process with an explicit status code.
- `abort` terminates the process immediately as abnormal termination.
- Neither `exit` nor `abort` runs caller-scope Nocter cleanup. Code that needs cleanup must do it before calling them.
- The target implementation uses the active target's syscall or process termination boundary.
- If the platform termination operation unexpectedly returns, the implementation calls `trap()`.
- The module path is `std/process`; target-dependent helper or primitive calls inside it are reached through `#target`-gated std internals.

### Pointer API

Adopted: v0.2.0 exposes only a narrow raw-pointer surface.

Initial public surface:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
```

Rules:

- These APIs are ordinary public standard-library primitive declarations under
  `std/ptr`.
- Pointer dereference and general user memory mutation through raw pointers are
  deferred.
- Raw construction helpers such as `from_addr`, raw-parts string/slice
  construction, raw stores, and raw copies are `pub(nocter)` std-internal APIs.

### Not Adopted

`std/posix` is not part of the initial design. macOS and Linux can share POSIX-like ideas, but Windows does not fit that layer cleanly. Shared concepts should use stable module paths such as `std/os`, `std/io`, and `std/process`. Their target-dependent boundaries should remain in std-internal modules with `#target` on type, helper, and primitive declarations.

## Standard Library and Low-Level Code

The compiler must not special-case names such as `print`, `args`, `env`, `cwd`, `exit`, `abort`, or `File`.

Standard library functions provide these features.

```nct
use std/io.stdout

func main(): i32! {
    var out = stdout()
    out.write_text("Hello\n")?
    return 0
}
```

The standard library may use typed `primitive` declarations to connect Nocter code to compiler-provided low-level implementations. Arbitrary inline ARM64 `asm` is not part of the initial language.

```nct
#target: "arm64-darwin"
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

#target: "arm64-darwin"
pub(nocter) primitive syscall3(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive trap(): never

#target: "arm64-darwin"
pub(nocter) primitive unreachable(): never
```

`trap()` terminates the current process or thread with a target-defined illegal-instruction, breakpoint, or equivalent non-recoverable stop. It has type `never`, does not return an error, and does not unwind.

`unreachable()` is a standard-library marker for paths that should be impossible. Reaching it traps.

Initial policy:

- `primitive` declarations are allowed only inside the active Nocter home `std/` directory in the initial design.
- A primitive declaration has no function body.
- A primitive declaration uses normal Nocter parameter and return types.
- Target-independent declarations do not use `#target`.
- Target-dependent type declarations, helper functions, and primitive declarations must be preceded by `#target: "target-name"`.
- `#target` applies only to top-level function, primitive, struct, enum, interface, and type-alias declarations in v0.2.0.
- `target` is not a reserved keyword; it is treated as the directive name only after `#`.
- Primitive calls follow normal visibility rules. A `pub` primitive may be called by any module that can import it. A `pub(nocter)` primitive may be called only inside the active Nocter home.
- Primitive calls follow the Nocter ABI after visibility and trusted-boundary restrictions pass.
- The compiler validates each primitive declaration against the target-independent core primitive set or the closed primitive set for the active target.
- The compiler validates primitives by module path, name, and exact signature.
- An ordinary `func` with the same name as a primitive has no primitive behavior.
- Standard-library wrappers should expose user-facing APIs such as `exit`, `write`, `write_text`, `alloc`, and `free`.
- `print`, `exit`, `abort`, file operations, allocators, `String`, and `Buffer` are not primitives in v0.2.0.
- General user code cannot declare primitives in v0.2.0. The long-term default direction is that user project modules do not declare primitives.
- General user code can call only the small public primitive APIs intentionally exposed by the standard library, such as safe raw-pointer address conversion. Target syscall primitives are `pub(nocter)`.
- Arbitrary inline `asm` is not part of the initial language.

### Trusted Boundary

Adopted: v0.2.0 has no `unsafe` keyword, no `unsafe` block, and no `unsafe func`.

Instead, the trusted boundary is the active Nocter home:

```text
~/.nocter/std/
```

Rules:

- User project modules are always safe Nocter code in v0.2.0.
- `unsafe` is not a reserved keyword in v0.2.0.
- `trusted` is not a reserved keyword in v0.2.0.
- Modules inside the active Nocter home may contain primitive declarations when those declarations match the closed primitive set.
- Modules inside the active Nocter home may call `pub(nocter)` primitive declarations.
- Modules inside the active Nocter home may call restricted low-level APIs such as `std/ptr.from_addr`.
- User project modules must not declare primitives.
- User project modules must not call `pub(nocter)` primitive declarations.
- User project modules must not call restricted low-level APIs such as `std/ptr.from_addr`.
- Trusted modules still go through normal parsing, type checking, ownership checking, borrowing rules, and drop checking.
- Trusted modules should expose ordinary safe APIs to user code, using types such as `File`, `String`, `Vec<T>`, `Allocator`, `error`, `&str`, `&[T]`, and `&+[T]`. Std-internal records such as `OSError` must stay behind `pub(nocter)` unless the public API contract is explicitly changed.
- If trusted standard-library code violates an invariant required by its public safe API, that is a standard-library or compiler bug. It is not an opt-in source-level permission granted to user code.

Initial primitive declaration syntax:

```nct
pub primitive name(params): ReturnType
pub(nocter) primitive name(params): ReturnType

#target: "arm64-darwin"
pub(nocter) primitive target_dependent_name(params): ReturnType
```

Initial primitive declaration modules:

```text
~/.nocter/std/error.nct
~/.nocter/std/ptr.nct
~/.nocter/std/string.nct
~/.nocter/std/io.nct
~/.nocter/std/os.nct
~/.nocter/std/process.nct
```

`std/error.nct` contains the target-independent built-in error payload construction primitive used by `Error.new`.

`std/ptr.nct` contains target-independent core pointer primitive declarations. These are required for raw pointer address conversion, borrow-to-pointer conversion, and std-internal construction and copying of string, byte, and typed views over raw storage.

`std/string.nct` contains the target-independent `bytes_from_str` primitive used to expose `&str` storage as a byte slice inside the standard library.

`std/io.nct` contains the initial `arm64-darwin` raw file descriptor primitives used by `File.open`, `File.read`, `File.write`, `File.write_text`, and `print`. These declarations are `pub(nocter)` and are not part of the user-facing I/O API.

`std/os.nct` contains common OS declarations plus target-specific declarations for `arm64-darwin` behind `#target: "arm64-darwin"`. Future OS targets should add new `#target: "..."` declarations inside stable std-internal modules instead of changing import resolution.

`std/process.nct` contains the initial `arm64-darwin` process termination primitive used by `std/process.exit`. It is `pub(nocter)` and is not part of the user-facing process API.

Initial core error primitive set:

```nct
pub(nocter) primitive new_error(code: &str, message: &str): error
```

Initial core pointer primitive set:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive pointee_size<T>(pointer: *T): usize
pub(nocter) primitive pointee_align<T>(pointer: *T): usize
pub(nocter) primitive copy_str_to_ptr(destination: *u8, offset: usize, text: &str): void
pub(nocter) primitive copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
pub(nocter) primitive store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
pub(nocter) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
pub(nocter) primitive slice_from_raw_parts(pointer: *u8, len: usize): &[u8]
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
pub(nocter) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
pub(nocter) primitive slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
```

`from_addr`, pointee layout queries, raw-storage copy/store helpers, and raw-storage
view construction helpers are `pub(nocter)` and therefore restricted to trusted
modules inside the active Nocter home. User project modules must not call them.
Calls to `from_addr` with an address expression statically known to be zero are
rejected because v0.2.0 raw pointers are non-null.

Initial core string primitive set:

```nct
pub(nocter) primitive bytes_from_str(value: &str): &[u8]
```

`bytes_from_str` is `pub(nocter)` and is exposed to user code only through ordinary standard-library wrappers such as `std/string.bytes` and `text.bytes()`.

Initial `arm64-darwin` target primitive set v0.2.0:

```nct
#target: "arm64-darwin"
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

#target: "arm64-darwin"
pub(nocter) primitive syscall0(number: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall1(number: usize, a0: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult

#target: "arm64-darwin"
pub(nocter) primitive trap(): never

#target: "arm64-darwin"
pub(nocter) primitive unreachable(): never

#target: "arm64-darwin"
pub(nocter) primitive open_read_raw(path: *u8): i32!

#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

#target: "arm64-darwin"
pub(nocter) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!

#target: "arm64-darwin"
pub(nocter) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!

#target: "arm64-darwin"
pub(nocter) primitive close_fd_raw(fd: i32): void

#target: "arm64-darwin"
pub(nocter) primitive exit_raw(code: i32): never
```

`SyscallResult.errno == 0` means success. `SyscallResult.errno != 0` means the syscall failed with an OS error value. The meaning of `value` is syscall-specific.

The compiler recognizes these declarations only at their registered module path and target:

```text
std/os
std/io
std/process
```

An ordinary function named `syscall3` elsewhere is not primitive.

`syscall0` through `syscall6` are a bootstrap boundary for the initial macOS standard library. The `std/io.*_raw` and `std/process.exit_raw` primitives are narrow bootstrap boundaries for shipped v0.2.0 I/O and process termination before all wrappers can be expressed through the ordinary `SyscallResult` path. Longer term, target-gated std internals may replace direct syscall exposure and these raw primitives with narrower typed wrappers. Those wrappers are standard-library APIs, not compiler primitives.

### Typed Primitive Wrappers

Adopted: typed wrappers over low-level target operations are standard-library APIs, not compiler primitives.

The closed compiler primitive set stays small. For the initial `arm64-darwin` target, the target primitive boundary is `std/os.syscall0` through `std/os.syscall6`, `std/os.trap`, `std/os.unreachable`, the raw `std/io` file descriptor primitives, and `std/process.exit_raw`; separately, `std/ptr`, `std/error`, and `std/string` own the target-independent core primitive set.

Std-internal modules may define narrower typed wrappers around those primitives:

```nct
pub(nocter) func write_fd(fd: FileDescriptor, bytes: &[u8]): void! {
    ...
}
```

User-facing modules then expose safe ordinary APIs:

```nct
pub method &+self.write(bytes: &[u8]): void! {
    ...
}
```

Rules:

- Adding a file API, process API, allocator API, string API, buffer API, or OS wrapper must not require adding a compiler primitive.
- The existing `std/io.*_raw` and `std/process.exit_raw` primitives are v0.2.0 bootstrap exceptions for shipped I/O and process termination, not a precedent for adding one primitive per standard-library API.
- Target-specific syscall numbers, raw OS handles, errno-like values, and calling conventions belong in target-gated std internals, not in the compiler's general language semantics.
- The compiler validates the existing primitive declarations by module path, name, and exact signature.
- An ordinary wrapper name such as `open_file_raw`, `write_fd_raw`, `mmap_raw`, or `exit_process` has no compiler-defined behavior.
- A future compiler primitive may be added only by an explicit language and backend design update, not as the normal way to grow the standard library.
- User project modules remain outside the primitive declaration boundary. If Nocter later adds an explicit trusted or unsafe extension, it should not make arbitrary user-defined primitive declarations the default extension mechanism.

## Keyword Ownership

Keyword and lexical rules are specified only in
[Lexical Grammar](13-lexical-grammar.md). This chapter does not maintain a
separate keyword list.

## Open Design Questions

No open design questions are currently listed in this chapter.
