# Standard-Library Primitive and OS Boundary

This chapter defines the boundary between ordinary standard-library source and compiler-owned
operations. User-facing library behavior is divided by responsibility:

- [Memory, Regions, and Allocators](../language/memory-and-regions.md) defines allocation policy and
  storage provenance;
- [Strings, Arrays, Views, and Pointers](../language/sequences-and-text.md) defines core memory
  views and owned text/collection behavior;
- [Callable Values and Interface Default Methods](../language/callables.md) defines callable and
  interface-default semantics;
- [Native Testing](../tooling/testing.md) defines test declarations and execution;
- [Standard Library](../../development/std/README.md) defines text, collection, path,
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

A primitive function may publish `noalloc` only when the closed compiler primitive registry
certifies that exact declaration role as allocation-free. Source spelling cannot upgrade an
unknown primitive effect. The checker consumes the registry fact through declaration identity and
uses the same callable-effect authority as ordinary bodies; the backend does not reinterpret the
modifier.

Every named built-in type has one `primitive type` declaration selected by exact source identity.
That declaration's module owns ordinary source-defined instances and construction for the type:
`char` is declared and owned by `std/char`, `str` by `std/str`, `error` by `std/error`, and boolean
and integer types by `std/num`. `void` and `never` are declared in `std/core` but admit no inherent
surface. Structural slices remain owned by the exact compiler-selected `std/slice` module because
`[T]` is a type constructor rather than a named declaration. Interface implementations are owned
by the selected standard-library package because an interface and the built-in's inherent surface
may have separate module responsibilities. A project package cannot declare or directly extend a
compiler-owned type. Authority is based on exact selected declarations and module identities, not
an arbitrary textual `std` prefix.

## Primitive Type Declarations

`primitive type` declares the source surface of one compiler-defined named type:

```nct
pub primitive type BuiltinName
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

The compiler-level failure payload is lowercase `error`. The exact type and member declarations
belong to the compiler-checked [`std/error` contract](../../development/std/error/index.nct), while
its storage, code, and context behavior belongs to [Recoverable Errors](../../development/std/error/README.md).
The language-level `T!` meaning and propagation rules remain in
[Errors and Optionals](../language/errors-and-optionals.md).

Public standard-library APIs expose target failures only through `error`. Target-specific raw
records and conversion helpers do not cross the public boundary and are not part of this public
specification.

## Primitive Function Declarations

A primitive function is an ordinary function contract whose implementation is supplied by the
toolchain. The `primitive` modifier precedes `func`; the declaration has a typed signature but no
Nocter body. It may be private when only its authored implementation source needs the trusted
operation:

```nct
primitive func platform_operation(input: usize): usize
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

The compiler-checked [`std/ptr` contract](../../development/std/ptr/index.nct) owns the exact public
pointer primitive declarations. Their observable behavior belongs to
[Pointer and Address Conversion](../../development/std/ptr/README.md), while raw-pointer language
semantics belong to [Strings, Arrays, Views, and Pointers](../language/sequences-and-text.md).
Package-internal raw-view construction retains separate trusted authority and is not made public by
the existence of the conversion API.

## Target-Gated Standard-Library Declarations

Target-dependent functions, primitives, aliases, structs, enums, and interfaces use a preceding
`#target` directive:

```nct
#target: "arm64-darwin"
primitive func target_operation(): void
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

Target primitives expose only the minimum facts needed by ordinary wrappers. The public
[`std/io`](../../development/std/io/README.md), [`std/fs`](../../development/std/fs/README.md), and
[`std/process`](../../development/std/process/README.md) guides own validation, handle ownership,
retry, partial-transfer, operation, and failure behavior. Their package-internal target translation
is an implementation contract rather than a second public API.

The compiler-generated entry wrapper may use the registered process boundary directly, but this
does not make similarly named user functions special or move public process policy into the
compiler.

## Keyword Ownership

Standard-library evolution does not reserve ordinary API names. Adding a type or function to
`std/` cannot change how an existing user identifier is parsed. Language keywords are listed only
in [Lexical Grammar](../language/lexical-grammar.md); standard-library names follow normal imports,
visibility, and collision rules.
