# Standard Library, Primitives, and OS

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## OS Error Model

Adopted: Nocter uses a layered OS error model. Target-specific raw errors are converted into common standard-library errors, then into domain-specific user-facing errors.

Layering:

```text
std.os.macos        target overlay: syscall, SyscallResult, Errno, errno mapping
std.os              common std: Platform, OSErrorKind, OSError
std.io              user-facing I/O errors and APIs
std.process         user-facing process APIs
```

The compiler must not special-case names such as `OSError`, `IOError`, `Errno`, `File`, or `exit`. These are ordinary standard-library names.

### Target Raw Errors

`std.os.macos` owns the raw macOS syscall result and errno wrapper.

```nct
pub copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub copy struct Errno {
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
std.os
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
- Common `std.os` does not define `Errno`.

Target overlays convert raw target errors into `OSError`.

```text
SyscallResult -> Errno -> OSError
```

### I/O Errors

User-facing I/O APIs return `std.io.IOError`, not `SyscallResult` or `Errno`.

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
std.os.macos.syscall3
    -> SyscallResult
    -> Errno
    -> std.os.OSError
    -> std.io.IOError
```

### Process Exit

`std.process.exit` is a normal standard-library API.

```nct
pub func exit(code: i32): never
```

Rules:

- `exit` is not a compiler primitive.
- `exit` does not return an error.
- The target implementation uses the active target's syscall or process termination boundary.
- If the platform exit operation unexpectedly returns, the implementation calls `trap()`.
- The module name is `std.process`, but the physical implementation may live in the active target overlay when the implementation depends on process ABI.

### Not Adopted

`std.posix` is not part of the initial design. macOS and Linux can share POSIX-like ideas, but Windows does not fit that layer cleanly. Shared concepts should use stable module names such as `std.os`, `std.io`, and `std.process`. Their physical implementation may live in common `std/` or in the active target overlay depending on whether the implementation is target-independent.

## Standard Library and Low-Level Code

The compiler must not special-case names such as `print`, `exit`, or `File`.

Standard library functions provide these features.

```nct
import std.io.stdout

program(): i32 {
    var out = stdout()
    out.write("Hello\n").ignore()
    return 0
}
```

The standard library may use typed `primitive` declarations to connect Nocter code to compiler-provided low-level implementations. Arbitrary inline ARM64 `asm` is not part of the initial language.

```nct
pub copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub primitive syscall3(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): SyscallResult

pub primitive trap(): never
pub primitive unreachable(): never
```

Initial policy:

- `primitive` declarations are allowed only inside the active Nocter home common `std/` directory or the active target overlay `std/` directory in the initial design.
- A primitive declaration has no function body.
- A primitive declaration uses normal Nocter parameter and return types.
- Primitive calls follow the Nocter ABI.
- The compiler validates each primitive declaration against the target-independent core primitive set or the closed primitive set for the active target.
- The compiler validates primitives by module path, name, and exact signature.
- An ordinary `func` with the same name as a primitive has no primitive behavior.
- Standard-library wrappers should expose user-facing APIs such as `exit`, `write`, `alloc`, and `free`.
- `print`, `exit`, file operations, allocators, `String`, and `Buffer` are not primitives in v0.
- General user code cannot declare primitives in the initial design.
- Arbitrary inline `asm` is not part of the initial language.

Initial primitive declaration syntax:

```text
pub primitive name(params): ReturnType
```

Initial primitive files:

```text
.nocter-arm64-macos/std/ptr.nct
.nocter-arm64-macos/targets/arm64-macos/std/os/macos.nct
```

`std/ptr.nct` contains target-independent core pointer primitive declarations. These are required for raw pointer address conversion and borrow-to-pointer conversion.

`std/os/macos.nct` is target-specific for `arm64-macos` and is loaded from the `arm64-macos` target overlay. Future OS targets should add separate target overlays instead of changing the language-level primitive syntax.

Initial core pointer primitive set:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub primitive from_addr<T>(address: usize): *T
```

`from_addr` is restricted to modules inside the active Nocter home. User project modules must not call it.

Initial `arm64-macos` target primitive set v0:

```nct
pub copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub primitive syscall0(number: usize): SyscallResult
pub primitive syscall1(number: usize, a0: usize): SyscallResult
pub primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult
pub primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
pub primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
pub primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
pub primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
pub primitive trap(): never
pub primitive unreachable(): never
```

`SyscallResult.errno == 0` means success. `SyscallResult.errno != 0` means the syscall failed with an OS error value. The meaning of `value` is syscall-specific.

The compiler recognizes these declarations only at their target-overlay module path:

```text
std.os.macos
```

An ordinary function named `syscall3` elsewhere is not primitive.

`syscall0` through `syscall6` are a bootstrap boundary for the initial macOS standard library. Longer term, target overlays may replace direct syscall exposure with narrower typed wrappers, but those wrappers should remain standard-library APIs unless a concrete compiler primitive is necessary.

## Reserved Keywords

Initial reserved keywords:

```text
import
program
func
pub
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
as
region
using
primitive
void
never
```

`program` is reserved because it is a top-level entry construct, not a normal identifier.

## Open Design Questions

The following areas remain intentionally open:

- exact grammar for generics
- exact generic parameter grammar beyond simple `T: Trait`
- full trait method-resolution order and ambiguity diagnostics
- collection iteration syntax and protocol
- optional propagation syntax, if any
- borrowed optional projections for `if let`
- package layout and multi-file module resolution
- detailed lifetime inference and borrow-checker diagnostics beyond the adopted core rules
- whether attributes are needed later
- future typed primitive wrappers beyond the initial `arm64-macos` syscall bootstrap set
- whether user-defined primitive declarations can ever exist outside the active Nocter home common `std/` or active target overlay `std/`
