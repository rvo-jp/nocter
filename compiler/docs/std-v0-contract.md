# Nocter Std v0 Contract

This document fixes the public standard-library contract for Nocter v0.
It is the implementation-facing companion to `../../spec/11-stdlib-primitives-os.md`
and the closure gates in `v0-closure.md`.

The goal is a small distributable `std/` that is stable enough for user code and
future implementation work. A public API listed here must not silently succeed
as a placeholder. It either works, fails with a documented `error`, or is not
part of the v0 public surface.

## Status Terms

- `runtime ship`: user code may import, build, run, and rely on this API on the
  v0 `arm64-darwin` target.
- `recoverable unsupported`: user code may import and type-check this API, but
  calling it before implementation must fail with `error` using the documented
  code. It must not return a fake success value.
- `check only`: the name is present to keep future API shape stable, but v0 does
  not promise native lowering for useful calls yet. User code outside the
  buildable subset may still be rejected before backend emission.
- `std internal`: visible only to the active Nocter home through `pub(nocter)`.
  User project modules must not import or call it.
- `defer`: not part of the v0 public contract.

## Distribution Rules

The v0 distribution contains these stable source files:

```text
.nocter/std/error.nct
.nocter/std/fmt.nct
.nocter/std/io.nct
.nocter/std/mem.nct
.nocter/std/os.nct
.nocter/std/prelude.nct
.nocter/std/process.nct
.nocter/std/ptr.nct
.nocter/std/string.nct
.nocter/std/vec.nct
```

Target-dependent declarations stay in those files behind `#target("...")`.
The distribution must not require target-specific file names such as
`std/os/macos.nct`.

Public APIs are ordinary Nocter APIs. The compiler must not special-case names
such as `String`, `Vec`, `File`, `Allocator`, `print`, `args`, `env`, `cwd`,
`exit`, or `abort`.

## Prelude

`std/prelude` is synthetic for user project modules and intentionally small.

```nct
pub use std/error.{Error, ErrorCode}
pub use std/string.String
```

Rules:

- `Int` is not part of v0. User code should write `i32` or define its own alias.
- `Vec` is not re-exported by the prelude in v0. Code that mentions `Vec<T>`
  must import `std/vec.Vec` explicitly.
- `File`, `Allocator`, `RawBuffer`, `print`, `stdout`, `stderr`, `args`, `env`,
  `cwd`, `exit`, and `abort` must be imported from their domain modules.

## Module Contract

### `std/error`

| API | Status | Notes |
|---|---|---|
| `pub type ErrorCode = &str` | runtime ship | Open string classification. |
| `pub type Error = error` | runtime ship | Alias for the built-in failure payload. |
| `pub func Error.new(code: ErrorCode, message: &str): Error` | runtime ship | Constructs the built-in payload. |
| `pub(nocter) primitive new_error(code: &str, message: &str): error` | std internal | Closed primitive boundary. |

### `std/string`

| API | Status | Notes |
|---|---|---|
| `pub struct String` | runtime ship | Move-only owned UTF-8 storage with private fields. |
| `String.empty`, `empty` | runtime ship | Returns an owned empty string. |
| `String.with_capacity`, `with_capacity` | runtime ship | Explicit allocator argument. |
| `String.from_str`, `from_str`, `String.copy` | runtime ship | Copies from `&str` into owned storage. |
| `String.view()` method, `view` | runtime ship | Returns `&str` view over owned storage. |
| `String.len()`, `String.capacity()`, `String.is_empty()` methods plus wrappers | runtime ship | Metadata accessors. |
| `String.reserve()`, `String.clear()` methods plus wrappers | runtime ship | Mutating owned string operations. |
| `String.push_str()` method, `push_str` | runtime ship | Appends a `&str`. |
| `String.bytes()` method, `bytes` | runtime ship narrow | Byte views for std I/O and formatting. |
| `capacity_overflow` | runtime ship | Returns `std.string.capacity_overflow`. |
| `bytes_from_str` | std internal | Primitive boundary for byte view construction. |

Bare string interpolation is not a public std API. Until interpolation lowering
has an explicit allocator source, the buildability preflight rejects it.

### `std/fmt`

| API | Status | Notes |
|---|---|---|
| `append_str` | runtime ship | Appends `&str` to an existing `String`. |
| `append_string` | runtime ship | Appends `view(value)`. |
| `append_i32` | runtime ship | Decimal formatting for `i32`. |
| `append_usize` | runtime ship | Decimal formatting for `usize`. |
| `append_bool` | runtime ship | Appends `true` or `false`. |
| `unsupported` | runtime ship | Returns `std.fmt.unsupported`. |

Formatting functions never choose an allocator. Callers create the destination
`String` explicitly.

### `std/mem`

| API | Status | Notes |
|---|---|---|
| `Layout` | runtime ship narrow | Public copy record for size/alignment. |
| `RawBuffer` | runtime ship narrow | Public owned raw byte allocation record. |
| `Allocator` | runtime ship narrow | Public allocator handle with private fields. |
| `page_allocator` | runtime ship | Initial allocator factory. |
| `alloc`, `free` | runtime ship narrow | Page-backed allocation and release. |
| `RawBuffer.bytes()`, `RawBuffer.bytes_mut()` methods plus wrappers | runtime ship narrow | Slice views over buffer storage. |
| `RawBuffer.prefix()`, `RawBuffer.prefix_mut()` methods plus wrappers | runtime ship narrow | Checked prefix views. |
| `out_of_memory`, `invalid_argument` | runtime ship | Standard memory errors. |
| `alloc_pages`, `free_pages` | std internal | Target-gated implementation helpers. |

General allocator strategies, regions beyond the current explicit API, and
collection storage policies are deferred.

### `std/io`

| API | Status | Notes |
|---|---|---|
| `File` | runtime ship narrow | Move-only file/stream handle with private fields. |
| `open(path: &str): File!` | runtime ship narrow | Free-function wrapper for opening existing files for reading. |
| `File.open(path: &str): File!` | runtime ship narrow | Opens existing files for reading. |
| `File.read(buffer: &+[u8]): usize!` | runtime ship narrow | Reads into caller storage. |
| `File.write(bytes: &[u8]): void!` | runtime ship narrow | Writes all bytes or fails. |
| `File.write_text(text: &str): void!` | runtime ship | Writes UTF-8 text. |
| `stdout`, `stderr` | runtime ship | Borrowed process standard streams. |
| `print(text: &str): void!` | runtime ship | Writes to stdout without newline. |
| `write_text(file: &+File, text: &str): void!` | runtime ship | Free-function wrapper. |
| `unsupported` | runtime ship | Returns `std.io.unsupported`. |
| raw fd helpers, `IOError`, `from_os_error` | std internal | Not user-facing v0 API. |

File creation, append, truncate, seek, paths, buffering, async I/O, and
directory traversal are deferred.

### `std/process`

| API | Status | Notes |
|---|---|---|
| `exit(code: i32): never` | runtime ship | Terminates with status code. |
| `abort(): never` | runtime ship | Traps immediately. |
| `cwd(allocator: &+Allocator): String!` | runtime ship narrow | Returns the current working directory as caller-owned `String` on `arm64-darwin`; fails with `std.process.cwd_failed` if the target path cannot be retrieved. |
| `env(name: &str): &str?!` | check only | Future fallible-optional shape is reserved. Useful runtime requires nested fallible/optional return lowering and process context storage. It must not be implemented as a fake successful `none`. |
| `args(): Vec<&str>!` | check only | Future API shape is reserved; useful runtime requires real `Vec` and process context. Current body fails with `std.process.unsupported`. |
| `exit_raw`, cwd syscall helpers | std internal | Target-gated termination primitive and ordinary Nocter wrappers over the closed `std/os` syscall boundary. |

Future `args` and `env` results borrow process-context storage valid for the
whole program. `cwd` returns caller-owned `String` storage allocated through the
explicit allocator.

### `std/vec`

| API | Status | Notes |
|---|---|---|
| `Vec<T>` | check only | Future owned variable-length array type. |

`Vec<T>` is present to reserve the process API shape, but v0 has no public
collection operations. It is not part of the prelude.

### `std/ptr`

| API | Status | Notes |
|---|---|---|
| `addr<T>` | runtime ship narrow | Converts raw pointer to address. |
| `from_ref<T>`, `from_ref_mut<T>` | runtime ship narrow | Borrow-to-pointer conversions. |
| `from_addr`, string/slice raw-parts and store/copy primitives | std internal | Active-home trusted boundary only. |

Pointer dereference and general user memory mutation through pointers are
deferred.

### `std/os`

`std/os` is std-internal in v0. Public user-facing OS records are deferred until
there is a stable cross-target API. Current `Platform`, `OSErrorKind`,
`OSError`, `Errno`, `SyscallResult`, syscall primitives, `trap`, and
`unreachable` are `pub(nocter)` implementation details used by std modules.

## Growth Rules

New std APIs must be added in this order:

1. Add the API to this contract with one status term.
2. Add or update `.nocter/std` declarations.
3. Add distributed-home tests that prove the declared status.
4. Add Frontend/Backend/runtime support only for APIs classified as
   `runtime ship`.

Do not add target-specific files under `std/`. Add target-gated declarations to
the stable module path instead.
