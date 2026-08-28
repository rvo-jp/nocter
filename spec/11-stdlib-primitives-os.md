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
        -> narrowly visible std implementation
            -> private or package-visible registered primitive
                -> target backend or process boundary
```

Adding a public file, process, allocator, string, collection, or formatting operation must not by
itself require a new compiler primitive. A primitive is justified only when ordinary Nocter code
cannot express the operation, such as issuing a target syscall, converting a borrow to an address,
or constructing a view from trusted raw parts.

Every named built-in type has one `primitive type` declaration selected by exact source identity.
That declaration's module owns ordinary source-defined instances and construction for the type:
`str` is declared and owned by `std/str`, `error` by `std/error`, and boolean and integer types by
`std/num`. `void` and `never` are declared in `std/core` but admit no inherent surface. Structural
slices remain owned by the exact compiler-selected `std/slice` module because `[T]` is a type
constructor rather than a named declaration. Interface implementations are owned by the selected
standard-library package because an interface and the built-in's inherent surface may have
separate module responsibilities. A project package cannot declare or directly extend a
compiler-owned type. Authority is based on exact selected declarations and module identities, not
an arbitrary textual `std` prefix.

## Primitive Type Declarations

`primitive type` declares the source surface of one compiler-defined named type:

```nct
// std/num/index.nct
pub primitive type i32

// std/str/index.nct
pub primitive type str
```

Rules:

- A primitive type declaration has visibility, `primitive type`, and exactly one name. It has no
  generic parameters, body, fields, variants, requirements, source representation, or alias
  target.
- Only the exact declaration selected for one closed compiler built-in role is valid. The complete
  selected standard package must declare every named built-in type exactly once, and one
  declaration cannot satisfy multiple roles.
- The declaration binds its name to the compiler's pre-existing canonical type identity. It never
  allocates a nominal type identity and never makes structural construction available.
- The declaration owns source documentation and editor navigation for the built-in type. Tools do
  not synthesize a second declaration or documentation surface.
- Named built-in declarations form a compiler-managed type fallback visible in every standard,
  dependency, package, and single-file source. This fallback is independent of `use` and the
  standard prelude. Authored declarations cannot shadow its names.
- The declaration's exact module owns inherent `construct` and `instance` surfaces for that named
  built-in. No separate path- or spelling-derived attachment authority exists.
- `&T`, `*T`, `[T]`, `[T; N]`, `T?`, `T!`, callable types, and other structural type constructors
  are not primitive type declarations.

## Error Boundary

The compiler-level failure payload is lowercase `error`. `std/error` owns its source-backed
construction surface:

```nct
construct error {
    pub func new(code: &str, message: &str): Self {
        return new_error(code, message)
    }
}

instance error {
    pub method self.context(message: &str): Self
    pub method &self.code(): &str from self
    pub method &self.message(): &str from self
    pub method &self.has_code(code: &str): bool
}
```

The standard library defines no `Error` or `ErrorCode` compatibility alias. Error codes are open
`&str` values. Construction snapshots its text, context consumes the prior handle, and accessors
borrow the owned immutable node through their receiver.

Standard-library error codes use stable dotted names such as `"std.io.not_found"`,
`"std.mem.out_of_memory"`, and `"std.process.invalid_encoding"`. Package and application code may
define its own prefixes. Public standard-library APIs return `error` through `T!`; target-specific
raw error records do not cross the public boundary.

The dotted codes are the public contract. Standard-library helper functions that construct those
errors are private implementation details and are not exported merely to make the code stable.

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
    not_directory
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

## Primitive Function Declarations

A primitive function is an ordinary function contract whose implementation is supplied by the
toolchain. The `primitive` modifier precedes `func`; the declaration has a typed signature but no
Nocter body. It may be private when only its authored implementation source needs the trusted
operation:

```nct
primitive func new_error(
    code: &str,
    message: &str,
): error
```

After visibility checks, calls are type checked and use the Nocter ABI like ordinary calls. The
backend supplies the implementation identified by the declaration's canonical standard-library
module path, name, generic shape, parameter types, result type, target, and metadata.

Rules:

- Primitive functions are allowed only in the exact implicit standard-library package selected
  by the active Nocter home.
- Every primitive must match an entry in the compiler's closed registry exactly.
- The registry assigns each primitive one authorized exposure: source-private, package-visible, or
  public. The declaration's normalized language visibility must match that exposure.
- Source-private primitives are callable only in their authored source and a source that directly
  sees it, following the ordinary private-declaration rule. Primitive authority does not widen
  that access.
- Moving a registered declaration to another module or changing its signature is a compile error.
- A function without the `primitive` modifier has no primitive behavior.
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

Postfix `!` failure lowers to the same compiler trap boundary as other always-on safety checks. It
does not call formatting, stderr, `exit`, or `abort` APIs from the standard library.

The registry is an implementation inventory, not a second public standard library. Public
documentation should describe the wrapper contract, not encourage direct use of restricted
primitives.

## Pointer Boundary

Three target-independent pointer primitives are intentionally public:

```nct
pub primitive func addr<T>(pointer: *T): usize
pub primitive func from_ref<T>(value: &T): *T
pub primitive func from_ref_mut<T>(value: &+T): *T
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
primitive func exit_raw(code: i32): never
```

Rules:

- The directive applies only to the immediately following function, primitive, or type
  declaration.
- It does not apply to `use`, `test`, `construct`, or `instance` declarations.
- The declaration participates in name resolution and compilation only when the selected target
  matches exactly.
- A gate name outside the compiler release's recognized target set is an error. A recognized but
  unimplemented target name remains valid in a gate and is rejected only when selected for a
  target program.
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
process termination, and generic syscall results. File-descriptor operations are ordinary
`std/io` functions over that syscall result boundary, not parallel compiler primitives.
`std/process`, `std/io`, and `std/fs` own public validation policy, ownership, retry policy,
partial-transfer handling, operation semantics, and public types. Package-internal helpers may
share a target path argument representation and I/O error mapping. `std/internal/os` remains the
dependency-free owner of syscall, errno-classification, and native-layout facts; it does not own
public operation policy.

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
