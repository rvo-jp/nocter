# Standard Library, Primitives, and OS

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## OS Error Model

Adopted: Nocter uses a layered OS error model. Target-specific raw errors are converted into common standard-library errors, then into domain-specific user-facing errors.

Layering:

```text
std/os/macos        target overlay: syscall, SyscallResult, Errno, errno mapping
std/os              common std: Platform, OSErrorKind, OSError
std/io              user-facing I/O errors and APIs
std/process         user-facing process APIs
```

The compiler must not special-case names such as `OSError`, `IOError`, `Errno`, `File`, or `exit`. These are ordinary standard-library names.

### Target Raw Errors

`std/os/macos` owns the raw macOS syscall result and errno wrapper.

```nct
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub(nocter) copy struct Errno {
    pub code: i32
}
```

Rules:

- `SyscallResult.errno == 0` means success.
- `SyscallResult.errno != 0` means failure.
- `SyscallResult.value` is syscall-specific.
- `Errno` is a target-overlay raw error wrapper.
- `Errno` is not exposed as the common OS error type.
- syscall number constants live in the target overlay, not common `std`.
- the exact syscall number list is a target implementation detail.

### Common OS Error

Common standard library module:

```text
std/os
```

Initial public surface:

```nct
pub enum Platform {
    macos
    linux
    windows
}

pub enum OSErrorKind {
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

pub copy struct OSError {
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
- Common `std/os` does not define `Errno`.

Target overlays convert raw target errors into `OSError`.

```text
SyscallResult -> Errno -> OSError
```

### I/O Errors

User-facing I/O APIs return `std/io`'s `IOError`, not `SyscallResult` or `Errno`.

Initial public surface:

```nct
pub enum IOError {
    interrupted(error: OSError)
    would_block(error: OSError)
    not_found(error: OSError)
    permission_denied(error: OSError)
    already_exists(error: OSError)
    invalid_input(error: OSError)
    broken_pipe(error: OSError)
    timed_out(error: OSError)
    unsupported(error: OSError)
    unexpected_os_error(error: OSError)
}
```

Examples:

```nct
func open(path: StringView): File!IOError
func write(file: &+File, text: StringView): void!IOError
```

Conversion flow:

```text
std/os/macos.syscall3
    -> SyscallResult
    -> Errno
    -> std/os.OSError
    -> std/io.IOError
```

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
- The module path is `std/process`, but the physical implementation may live in the active target overlay when the implementation depends on process ABI.

### Not Adopted

`std/posix` is not part of the initial design. macOS and Linux can share POSIX-like ideas, but Windows does not fit that layer cleanly. Shared concepts should use stable module paths such as `std/os`, `std/io`, and `std/process`. Their physical implementation may live in common `std/` or in the active target overlay depending on whether the implementation is target-independent.

## Standard Library and Low-Level Code

The compiler must not special-case names such as `print`, `exit`, `abort`, or `File`.

Standard library functions provide these features.

```nct
from std/io import stdout

program(): i32 {
    var out = stdout()
    out.write("Hello\n").ignore()
    return 0
}
```

The standard library may use typed `primitive` declarations to connect Nocter code to compiler-provided low-level implementations. Arbitrary inline ARM64 `asm` is not part of the initial language.

```nct
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub(nocter) primitive syscall3(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): SyscallResult

pub(nocter) primitive trap(): never
pub(nocter) primitive unreachable(): never
```

`trap()` terminates the current process or thread with a target-defined illegal-instruction, breakpoint, or equivalent non-recoverable stop. It has type `never`, does not return an error, and does not unwind.

`unreachable()` is a standard-library marker for paths that should be impossible. Reaching it traps.

Initial policy:

- `primitive` declarations are allowed only inside the active Nocter home common `std/` directory or the active target overlay `std/` directory in the initial design.
- A primitive declaration has no function body.
- A primitive declaration uses normal Nocter parameter and return types.
- Primitive calls follow normal visibility rules. A `pub` primitive may be called by any module that can import it. A `pub(nocter)` primitive may be called only inside the active Nocter home.
- Primitive calls follow the Nocter ABI after visibility and trusted-boundary restrictions pass.
- The compiler validates each primitive declaration against the target-independent core primitive set or the closed primitive set for the active target.
- The compiler validates primitives by module path, name, and exact signature.
- An ordinary `func` with the same name as a primitive has no primitive behavior.
- Standard-library wrappers should expose user-facing APIs such as `exit`, `write`, `alloc`, and `free`.
- `print`, `exit`, `abort`, file operations, allocators, `String`, and `Buffer` are not primitives in v0.
- General user code cannot declare primitives in v0. The long-term default direction is that user project modules do not declare primitives.
- General user code can call only the small public primitive APIs intentionally exposed by the standard library, such as safe raw-pointer address conversion. Target syscall primitives are `pub(nocter)`.
- Arbitrary inline `asm` is not part of the initial language.

### Trusted Boundary

Adopted: v0 has no `unsafe` keyword, no `unsafe` block, and no `unsafe func`.

Instead, the trusted boundary is the active Nocter home:

```text
~/.nocter/std/
~/.nocter/targets/<target>/std/
```

Rules:

- User project modules are always safe Nocter code in v0.
- `unsafe` is not a reserved keyword in v0.
- `trusted` is not a reserved keyword in v0.
- Modules inside the active Nocter home may contain primitive declarations when those declarations match the closed primitive set.
- Modules inside the active Nocter home may call `pub(nocter)` primitive declarations.
- Modules inside the active Nocter home may call restricted low-level APIs such as `std/ptr.from_addr`.
- User project modules must not declare primitives.
- User project modules must not call `pub(nocter)` primitive declarations.
- User project modules must not call restricted low-level APIs such as `std/ptr.from_addr`.
- Trusted modules still go through normal parsing, type checking, ownership checking, borrowing rules, and drop checking.
- Trusted modules should expose ordinary safe APIs to user code, using types such as `File`, `String`, `Buffer<T>`, `OSError`, `IOError`, `Allocator`, `View<T>`, and `WriteView<T>`.
- If trusted standard-library code violates an invariant required by its public safe API, that is a standard-library or compiler bug. It is not an opt-in source-level permission granted to user code.

Initial primitive declaration syntax:

```text
pub primitive name(params): ReturnType
pub(nocter) primitive name(params): ReturnType
```

Initial primitive files:

```text
~/.nocter/std/ptr.nct
~/.nocter/targets/arm64-macos/std/os/macos.nct
```

`std/ptr.nct` contains target-independent core pointer primitive declarations. These are required for raw pointer address conversion and borrow-to-pointer conversion.

`std/os/macos.nct` is target-specific for `arm64-macos` and is loaded from the `arm64-macos` target overlay. Future OS targets should add separate target overlays instead of changing the language-level primitive syntax.

Initial core pointer primitive set:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub(nocter) primitive from_addr<T>(address: usize): *T
```

`from_addr` is `pub(nocter)` and therefore restricted to trusted modules inside the active Nocter home. User project modules must not call it.

Initial `arm64-macos` target primitive set v0:

```nct
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub(nocter) primitive syscall0(number: usize): SyscallResult
pub(nocter) primitive syscall1(number: usize, a0: usize): SyscallResult
pub(nocter) primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult
pub(nocter) primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
pub(nocter) primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
pub(nocter) primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
pub(nocter) primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
pub(nocter) primitive trap(): never
pub(nocter) primitive unreachable(): never
```

`SyscallResult.errno == 0` means success. `SyscallResult.errno != 0` means the syscall failed with an OS error value. The meaning of `value` is syscall-specific.

The compiler recognizes these declarations only at their target-overlay module path:

```text
std/os/macos
```

An ordinary function named `syscall3` elsewhere is not primitive.

`syscall0` through `syscall6` are a bootstrap boundary for the initial macOS standard library. Longer term, target overlays may replace direct syscall exposure with narrower typed wrappers. Those wrappers are standard-library APIs, not compiler primitives.

### Typed Primitive Wrappers

Adopted: typed wrappers over low-level target operations are standard-library APIs, not compiler primitives.

The closed compiler primitive set stays small. For the initial `arm64-macos` target, the OS primitive boundary is `syscall0` through `syscall6`, `trap`, and `unreachable`; separately, `std/ptr` owns the target-independent core pointer primitive set.

Target overlays may define narrower typed wrappers around those primitives:

```nct
pub(nocter) func write_fd(fd: FileDescriptor, bytes: View<u8>): void!OSError {
    ...
}
```

User-facing modules then expose safe ordinary APIs:

```nct
pub method (file: &+File).write(bytes: View<u8>): void!IOError {
    ...
}
```

Rules:

- Adding a file API, process API, allocator API, string API, buffer API, or OS wrapper must not require adding a compiler primitive.
- Target-specific syscall numbers, raw OS handles, errno-like values, and calling conventions belong in the target overlay, not in the compiler's general language semantics.
- The compiler validates the existing primitive declarations by module path, name, and exact signature.
- An ordinary wrapper name such as `open_file_raw`, `write_fd_raw`, `mmap_raw`, or `exit_process` has no compiler-defined behavior.
- A future compiler primitive may be added only by an explicit language and backend design update, not as the normal way to grow the standard library.
- User project modules remain outside the primitive declaration boundary. If Nocter later adds an explicit trusted or unsafe extension, it should not make arbitrary user-defined primitive declarations the default extension mechanism.

## Reserved Keywords

Initial reserved keywords:

```text
from
import
use
program
func
pub
type
copy
struct
enum
trait
impl
method
let
var
return
if
else
for
in
while
loop
break
continue
match
is
try
catch
fail
none
move
drop
as
region
using
primitive
void
never
```

`program` is reserved because it is a top-level entry construct, not a normal identifier.

`nocter` is not a reserved keyword. It is recognized only as the contextual visibility scope in `pub(nocter)`.

`@` is reserved for possible future attribute-like syntax, but attributes are not part of v0. A source-level `@` outside string literals, byte literals, or comments is invalid in v0.

## Open Design Questions

No open design questions are currently listed in this chapter.
