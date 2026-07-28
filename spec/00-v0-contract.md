# Nocter v0 Contract

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

This chapter defines the implementation contract for Nocter v0. It is narrower
than the long-term language direction. A feature that is not listed here is not
part of the v0 contract even if older notes or future-design sections mention
it.

## Interfaces And Excluded Features

Nocter v0 includes contract-only `interface` declarations and explicit
interface conformance declarations.

An interface contains only public method signatures. It cannot contain method
bodies, fields, associated data, default methods, associated types, or reusable
code.

```nct
pub interface Printable {
    pub method &self.print(): i32
}

impl Printable for User
```

Conformance is explicit and structural. `impl Printable for User` opts `User`
into the contract, and typechecking verifies that `User` has public inherent
methods matching each interface method after substituting `Self` with `User`.
The compiler does not infer accidental conformance from a matching shape alone.

The following features are not part of v0:

- `trait` declarations
- embedding declarations such as `...Type` and `pub ...Type`
- literal definitions such as `literal Vec<T> [...items: [T]]: Self`
- typed literal construction such as `Vec [1, 2, 3]`
- generalized `...` spread, rest capture, and variadic capture forms
- generic bounds such as `T: Interface`
- interface-bound method lookup
- dynamic dispatch and interface objects such as `dyn Printable`
- `where` clauses
- class inheritance
- code reuse through interfaces
- user-defined primitive declarations outside the trusted standard-library
  boundary

`trait` is not a reserved keyword in v0. It is lexed as an identifier. Source
forms that try to use trait syntax are diagnosed as removed syntax by the
parser. `interface` is a reserved keyword.

## Parser Contract

The v0 parser accepts these top-level item forms:

- `use path` as a module namespace import using the path's default name
- `use path as name` as a module namespace import using an explicit name
- `use path.name`
- `use path.{name_a, name_b}`
- `pub use path.name`
- `pub use path.{name_a, name_b}`
- `#target("target-name")` immediately before one eligible top-level declaration
- `primitive name<T>(...): Type`
- `pub primitive name<T>(...): Type`
- `pub(nocter) primitive name<T>(...): Type`
- `func name<T>(...): Type { ... }`
- `pub func name<T>(...): Type { ... }`
- `pub(nocter) func name<T>(...): Type { ... }`
- `func Type.name<T>(...): Type { ... }`
- `pub func Type.name<T>(...): Type { ... }`
- `pub(nocter) func Type.name<T>(...): Type { ... }`
- `type Name<T> = Type`
- `pub type Name<T> = Type`
- `pub(nocter) type Name<T> = Type`
- `copy struct Name<T> { ... }`
- `pub copy struct Name<T> { ... }`
- `pub(nocter) copy struct Name<T> { ... }`
- `struct Name<T> { ... }`
- `pub struct Name<T> { ... }`
- `pub(nocter) struct Name<T> { ... }`
- `enum Name<T> { ... }`
- `pub enum Name<T> { ... }`
- `pub(nocter) enum Name<T> { ... }`
- `interface Name<T> { pub method ... }`
- `pub interface Name<T> { pub method ... }`
- `pub(nocter) interface Name<T> { pub method ... }`
- `impl Type { method ...; drop ... }`
- `impl Interface for Type`
- `impl Interface for Type {}`

Top-level `use` and `pub use` forms are accepted only at the start of a source
file before non-`use` declarations.

`#target("target-name")` is a directive, not an attribute. It applies only to the
single top-level declaration that immediately follows it. In v0 that following
declaration must be a function, primitive, struct, enum, interface, or type alias.
The parser recognizes the directive form everywhere so it can produce useful
diagnostics, but target validation rejects `#target` outside the active Nocter
home or before an ineligible declaration.

The v0 parser also accepts the non-`pub` `use` forms above at the start of any
block scope. Block-scope `use` declarations are lexical declarations. They are
not runtime statements, and their imported names are visible only inside the
containing block after the declaration.

The v0 parser rejects these forms with diagnostics instead of accepting them as
partial language support:

- `trait Name { ... }`
- wildcard imports such as `use std/io.*` and `pub use std/io.*`
- source-level prelude imports such as `use std/prelude`
- textual include such as `include std/prelude`
- dotted module paths such as `use std.io.print`
- explicit `.nct` extensions in import paths such as `use ./config.nct.Config`
- relative or absolute path-like module expressions such as
  `./path/to/file.something()` and `/absolute/path/file.something()`
- top-level `use` after a non-`use` declaration
- block-scope `use` after a non-`use` statement or result expression
- block-scope `pub use`
- `struct Name { ...Type }`
- `struct Name { pub ...Type }`
- `literal Type [...]`
- `func name(...values: [T]): Type`
- `impl Interface for Type { method ... }`
- generic bounds such as `<T: Interface>`
- `func` declarations inside `impl`

The parser must not panic on malformed input. A parse failure must produce a
diagnostic with a source span.

## Resolver Contract

The resolver owns name binding for v0. Later stages, CLI diagnostics, and LSP
features must use resolver output instead of reimplementing lookup.

The resolver must classify each referenced name as one of:

- resolved global symbol
- resolved local symbol
- unresolved identifier diagnostic
- unsupported deferred syntax diagnostic emitted earlier by the parser

The resolver's v0 symbol space includes:

- functions
- primitives
- imported namespaces
- imported symbols
- type aliases
- structs
- enums
- interfaces
- associated functions attached to nominal types
- inherent methods attached to nominal types
- inherent `drop` members attached to nominal types
- local parameters and bindings

Trait symbols are not part of the v0 source-level contract. v0 source can
create interface symbols through the parser.

## Typecheck Contract

The typechecker is the source of truth for typed facts used by backend lowering,
CLI diagnostics, and LSP features.

For v0, typechecking must produce or diagnose:

- expression result types
- binding and parameter types
- readonly versus readwrite binding facts
- function, primitive, associated function, method, and drop signatures
- field access target and field type
- struct literal field checks
- enum variant construction and payload checks
- method call receiver type and selected inherent method
- explicit interface conformance against public inherent methods
- associated function call target
- move, copy, and drop state for owned values
- use-after-move and invalid explicit `drop` diagnostics
- unsupported construct diagnostics before backend lowering

The backend must not infer language semantics that are missing from typechecking.
If a source construct is not represented in the v0 typed facts, it must either
be added to this contract or rejected before lowering.

## Aggregate Type Contract

The aggregate contract covers structs, enums, optionals, fallible values, fixed
arrays, and standard-library owned aggregate types such as `String`.

Frontend facts required by backend and ABI lowering:

- resolved nominal type identity
- field order and field types
- enum variant order and payload types
- fixed-array element type and compile-time length
- `copy struct` marker
- whether a type has an inherent `drop` member
- expression ownership category: copy, move-only owned value, borrow, view,
  pointer, or unsized data behind indirection
- active move/drop state for each owned local place

Backend and ABI lowering may rely on these facts and must not repeat semantic
lookup. Lowering may compute target layout and calling convention details from
the typed facts.

Rules fixed for v0:

- struct fields are laid out in declaration order
- struct layout does not change when a `drop` member exists
- payloadless enum values use the explicit `u8` tag layout specified by ABI v0
- payload-carrying enum values are typechecked in v0 but rejected before
  build/run lowering until their runtime layout is promoted into ABI v0
- fixed arrays use contiguous element layout with a compile-time length
- optional and fallible values use explicit tags
- values of 16 bytes or less use direct ABI classification
- values larger than 16 bytes use indirect ABI classification
- drop glue must drop only live fields, initialized array elements, or active
  payloads
- moved-from owned places are dead until assigned a new value

## String Type Contract

`String` is an ordinary standard-library owned type, not a built-in type name.
The compiler must not treat the identifier `String` as magic.

The compiler-owned string types are:

- `str`: unsized UTF-8 byte data
- `&str`: borrowed UTF-8 slice with `ptr + len` ABI

String literals have type `&str`, point at static storage, and do not allocate.

The v0 `String` contract for compiler and standard library integration:

- `String` owns valid UTF-8 bytes
- `String` is move-only unless the standard library exposes an explicit copy API
- `String` releases its owned storage through its inherent `drop` member
- `String` can produce a borrowed `&str` view
- constructing a `String` from `&str` is explicit and fallible
- interpolation, if parsed, must not lower until the standard-library
  construction API is implemented

The runtime representation is a standard-library ABI contract, not user-facing
syntax. The initial implementation may use a pointer, length, and capacity
representation, but source code must interact through ordinary associated
functions and methods.

## Tooling Contract

CLI diagnostics and LSP features must use the same parser, resolver, and
typecheck facts as the compiler pipeline.

Required shared facts for tooling:

- source spans for declarations and references
- resolved symbol targets
- normalized type labels for identifiers and expressions
- function and method signature labels
- member and field targets
- diagnostic codes and source spans

LSP-specific code may translate these facts into protocol objects, but it must
not introduce separate language lookup rules.
