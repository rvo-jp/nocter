# Standard Library, Primitives, and OS

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

This chapter defines the boundary between ordinary standard-library source and compiler-owned
operations. User-facing library behavior is divided by responsibility:

- [Memory, Regions, and Allocators](06-memory-region-allocator.md) defines allocation policy and
  storage provenance;
- [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md) defines core memory
  views and owned text/collection behavior;
- [Callable Values and Interface Default Methods](18-callables-default-methods.md) defines iterator contracts;
- [Native Testing](20-native-testing.md) defines native tests and assertions;
- [Practical Standard Library](21-practical-standard-library.md) defines text, collection, path,
  file, numeric, and process APIs.

## Standard-Library Architecture

The installed Nocter home contains the standard library under `std/`. Its public types and
functions are ordinary Nocter declarations. The compiler must not assign intrinsic behavior to
names such as `String`, `Vec`, `File`, `Allocator`, `print`, `args`, `env`, `cwd`, `exit`,
or `abort`.

Compiler-owned behavior is restricted to built-in language types and operations, declaration
metadata consumed by compilation, and a closed primitive registry. Public wrappers remain in
Nocter source even when their implementation eventually reaches a primitive.

Representative layering:

```text
user package
    -> public std function or type
        -> package-visible std implementation
            -> registered primitive
                -> target backend or process boundary
```

Adding a public file, process, allocator, string, collection, or formatting operation must not by
itself require a new compiler primitive. A primitive is justified only when ordinary Nocter code
cannot express the operation, such as issuing a target syscall, converting a borrow to an address,
or constructing a view from trusted raw parts.

Built-in types may receive ordinary source-defined instances or construction only from the exact
implicit standard-library module recorded for that built-in identity. `str` is owned by
`std/str`, slices by `std/slice`, `error` construction by `std/error`, and scalar inherent APIs by
`std/num`. Interface conformances are owned by the selected standard-library package because an
interface and the built-in's inherent surface have separate module responsibilities. A project
package cannot add behavior directly to a compiler-owned type. This preserves one coherent source
surface without turning the built-in identity into a synthetic nominal declaration. Authority is
based on selected package and module identity, not an arbitrary textual `std` prefix.

## Error Boundary

The compiler-level failure payload is lowercase `error`. `std/error` owns its source-backed
construction surface:

```nct
construct error {
    pub default func new(code: &str, message: &str): Self from code | message {
        return new_error(code, message)
    }
}
```

The standard library defines no `Error` or `ErrorCode` compatibility alias. Error codes are open
`&str` values.

Standard-library error codes use stable dotted names such as `"std.io.not_found"`,
`"std.mem.out_of_memory"`, and `"std.process.invalid_encoding"`. Package and application code may
define its own prefixes. Public standard-library APIs return `error` through `T!`; target-specific
raw error records do not cross the public boundary.

`std/internal/os` converts target results through an internal common model:

```text
target result -> target raw error -> OSError -> public error
```

The current internal records are `pub(/)` declarations, visible throughout the implicit `std`
package:

```nct
pub(/) enum Platform {
    macos
    linux
    windows
}

pub(/) enum OSErrorKind {
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

pub(/) copy struct OSError {
    pub platform: Platform
    pub code: i32
    pub kind: OSErrorKind
}
```

Target-specific values preserve their raw numeric code, while `OSErrorKind` provides the portable
classification used by higher-level wrappers. Unknown target errors map to `unknown` rather than
being misclassified.

## Primitive Declarations

A primitive declaration has a typed Nocter signature but no Nocter body:

```nct
pub(/) primitive new_error(code: &str, message: &str): error
```

After visibility checks, calls are type checked and use the Nocter ABI like ordinary calls. The
backend supplies the implementation identified by the declaration's canonical standard-library
module path, name, generic shape, parameter types, result type, target, and metadata.

Rules:

- Primitive declarations are allowed only in the exact implicit standard-library package selected
  by the active Nocter home.
- Every primitive must match an entry in the compiler's closed registry exactly.
- Moving a registered declaration to another module or changing its signature is a compile error.
- An ordinary function with the same name has no primitive behavior.
- `pub(/)` primitives are callable only from modules in that same `std` package.
- A deliberately public primitive remains subject to normal import and type rules.
- User packages cannot declare primitives, even when they use the same module spelling or name.
- Primitive lowering must preserve Nocter safety, ownership, provenance, and failure contracts at
  its typed boundary.
- Arbitrary inline assembly and user-defined target intrinsics are not supported.

The closed registry has three broad responsibilities:

- target-independent representation bridges, including error creation, allocation-context state,
  pointer/address conversion, raw-part view construction, and trusted element movement;
- target-specific process and I/O boundaries;
- target-specific syscall, trap, and unreachable boundaries used by standard-library internals.

The registry is an implementation inventory, not a second public standard library. Public
documentation should describe the wrapper contract, not encourage direct use of restricted
primitives.

## Pointer Boundary

Three target-independent pointer primitives are intentionally public:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
```

They convert an existing pointer or borrow without granting dereference permission. Operations
that construct pointers or views from integer addresses and raw parts are `pub(/)` so only the
implicit `std` package can call them. Their primitive authority is independently tied to that
package's toolchain identity because their validity depends on invariants unavailable to general
source code. The complete user-facing pointer contract is in
[Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md).

## Target-Gated Standard-Library Declarations

Target-dependent functions, primitives, aliases, structs, enums, and interfaces use a preceding
`#target` directive:

```nct
#target: "arm64-darwin"
pub(/) primitive exit_raw(code: i32): never
```

Rules:

- The directive applies only to the immediately following function, primitive, or type
  declaration.
- It does not apply to `use`, `test`, `construct`, `instance`, or `conform` declarations.
- The declaration participates in name resolution and compilation only when the selected target
  matches exactly.
- A target-independent declaration has no `#target` directive.
- Directive names are contextual identifiers after `#`; `target` is not a reserved keyword.
- Target-specific constants, syscall numbers, calling conventions, and raw handles remain inside
  target-gated standard-library code.
- Public wrappers should be target-independent when they can provide the same contract on every
  supported target.

The current executable target is `arm64-darwin`. Recognizing a future target name does not make its
standard-library boundary buildable; the driver must reject targets whose backend, primitive
registry, and standard-library declarations are incomplete.

## Trusted Boundary

Nocter has no `unsafe` keyword, unsafe block, or unsafe function declaration. Trusted low-level
authority belongs to the exact implicit standard-library package selected by the active Nocter
home. Visibility remains an independent source-access rule.

Rules:

- User package modules are always ordinary safe Nocter code.
- `unsafe` and `trusted` are ordinary identifiers, not permission markers.
- Only the implicit toolchain standard-library package may declare registered primitives.
- Package-visible `pub(/)` declarations are accessible only from modules with the same exact
  package identity.
- A project directory named `std` cannot shadow the compiler-matched standard library or gain
  trusted authority.
- A dependency package cannot gain trusted authority from its package name or filesystem layout.
- Raw pointers in user code remain non-owning address values without dereference permission.
- The compiler validates the boundary before lowering; packaging or path tricks cannot defer this
  check to runtime.

This boundary keeps low-level implementation code reviewable without creating a general-purpose
escape hatch in the user language. If Nocter later needs third-party trusted code, that requires a
separate capability and distribution design rather than overloading visibility.

## Process and I/O Boundaries

Target primitives expose only the minimum facts needed by ordinary wrappers: process entry state,
file-descriptor operations, process termination, and generic syscall results. `std/process` and
`std/io` own validation, UTF-8 policy, ownership, retry policy, error mapping, and public types.

Consequences:

- entry parameters are accessed through `std/process`, not special `main` parameters;
- file handles are owned by `File`, not by compiler-known integers;
- allocation failure follows allocator policy, while I/O, path, encoding, and OS failures remain
  recoverable `T!` results;
- `exit` and `abort` are standard-library functions returning `never`;
- destruction cannot return an I/O error, so operations such as buffered flush that must report
  failure require an explicit call before drop.

The compiler-generated entry wrapper may use the registered process boundary directly, but this
does not make similarly named user functions special.

## Keyword Ownership

Standard-library evolution does not reserve ordinary API names. Adding a type or function to
`std/` cannot change how an existing user identifier is parsed. Language keywords are listed only
in [Lexical Grammar](13-lexical-grammar.md); standard-library names follow normal imports,
visibility, and collision rules.
