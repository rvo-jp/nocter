# Std Runtime Status

This document records the implementation status of the tracked
`development/std/` tree that is packaged as `std/` inside a Nocter home. The
public standard-library specification lives in
[Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md).

Do not add a public std API here first. Add or change the public contract in the
spec, then update `development/std/`, tests, and this implementation status.

## Distribution Files

The v0 distribution currently uses stable module files:

```text
development/std/error.nct
development/std/fmt.nct
development/std/io.nct
development/std/mem.nct
development/std/os.nct
development/std/prelude.nct
development/std/process.nct
development/std/ptr.nct
development/std/string.nct
development/std/vec.nct
```

Target-dependent declarations stay in these files behind `#target("...")`.
The distribution must not require target-specific source file names such as
`std/os/macos.nct`.

## Status Terms

- `runtime ship`: user code may import, build, run, and rely on this API on the
  v0 `arm64-darwin` target.
- `recoverable unsupported`: user code may import and call the API, but it
  fails with a documented `error` instead of returning fake success.
- `check-only`: the name is present for future API shape, but useful native
  lowering/runtime behavior is not shipped.
- `std internal`: visible only inside the active Nocter home through
  `pub(nocter)`.
- `private helper`: module-private implementation detail.
- `defer`: not part of the v0 public surface.

## Public Runtime Surface

| Module | Current status |
|---|---|
| `std/prelude` | Runtime-shipped synthetic prelude exports only `Error`, `ErrorCode`, and `String`. `Int` is removed. `Vec`, `File`, allocator, process, and I/O names require explicit `use`. |
| `std/error` | Runtime-shipped `ErrorCode`, `Error`, and `Error.new`. `new_error` is `pub(nocter)`. |
| `std/string` | Runtime-shipped owned `String` with private representation, explicit allocation, copy from `&str`, view, len/capacity/is_empty, reserve, clear, push_str, bytes, and drop. |
| `std/fmt` | Runtime-shipped append helpers for `&str`, `String`, `i32`, `usize`, and `bool`. Formatting APIs append into caller-owned `String` and do not choose an allocator. |
| `std/mem` | Runtime-shipped narrow page allocator, `Layout`, opaque `RawBuffer`, `Allocator`, alloc/free, bytes views, prefix views, and standard memory errors. `RawBuffer` storage fields are std-internal through `pub(nocter)`. |
| `std/io` | Runtime-shipped narrow `File`, `open`, `read`, `write`, `write_text`, stdout/stderr, `print`, close-on-drop state, and raw fd helpers behind `pub(nocter)`. |
| `std/process` | Runtime-shipped `exit`, `abort`, `cwd`, and `args` on `arm64-darwin`. `env(name): &str?!` is check-only. |
| `std/vec` | Runtime-shipped narrow `Vec<T>` construction, capacity, len, is_empty, reserve, push, from_slice, clear, readonly/readwrite views, and storage release for scalar, `&str`, and current copy-aggregate element paths. |
| `std/ptr` | Runtime-shipped public `addr`, `from_ref`, and `from_ref_mut` address conversions plus std-internal raw storage sizing, copy/store, and view-construction helpers. Pointer dereference is deferred. |
| `std/os` | Std-internal common OS records and target-gated syscall/trap primitives. No public user-facing OS module surface is shipped in v0. |

## Known Std Gaps

- `std/process.env` runtime implementation
- byte-to-UTF-8 validation boundaries for all future targets
- richer file modes, close API, seek, buffering, paths, directories, and async I/O
- non-copy aggregate `Vec<T>` element storage and per-element drop glue
- vector insertion/removal/iteration helpers
- allocator families beyond the current page-backed allocator
- public OS abstraction types
- interpolation APIs that can be used without an explicit allocator source

## Growth Rules

New std APIs must be added in this order:

1. Update `../../spec/11-stdlib-primitives-os.md` with the public contract.
2. Add or update `development/std/` declarations and implementations.
3. Add distributed-home tests for visibility, privacy, and status.
4. Add frontend/backend/runtime support only for APIs classified as
   `runtime ship`.
5. Update this document and `implementation-status.md`.

Placeholder APIs must not silently succeed. They either work, fail with a
documented `error`, remain check-only, or stay out of the public surface.
