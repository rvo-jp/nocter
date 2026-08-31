# Modules, Use Declarations, and Source Visibility

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Directory Modules

Nocter has no `module` declaration. Physical placement determines module ownership. A directory
containing `index.nct` defines one module, and the directory path defines that module's identity.

```text
project/
    index.nct              package root and root-module source
    app.nct                root-module source
    parser/
        index.nct          child module `parser`
        lexer.nct          child-module source
    internal/
        scanner.nct        root-module source in a source folder
```

Every `.nct` file belongs to the nearest ancestor directory containing `index.nct`. `app.nct` and
`internal/scanner.nct` therefore belong to the root module; `parser/lexer.nct` belongs to `parser`.
A directory without `index.nct` is a source folder, not a module or namespace. A source basename
never creates a module.

Selecting a module inventories all physical `.nct` files owned by that module. Inventory descends
through source folders and stops before a descendant directory containing `index.nct`, because that
directory starts another module or package. Source inventory and source visibility are independent:
an inventoried source is checked even when no other source writes `see` for it, but its declarations
remain private to that source until an authored `see` grants direct visibility.

`index.nct` is the module root source and the conventional public contract. Public APIs should be
declared there without substantial bodies. Trivial bodies are allowed, but nontrivial definitions
belong in ordinary module sources so `index.nct` remains readable as API documentation. This is a
style rule rather than a grammar restriction.

Import paths use `/` and omit `.nct`:

```text
/work/project/index.nct                    => /
/work/project/parser/index.nct             => /parser
~/.nocter/std/io/index.nct                  => std/io
```

## Module Imports

`use` is lexical compile-time syntax, not a runtime statement. Module imports may introduce a
namespace, select public type names, or define public re-exports:

```nct
use std/io
use std/io as console
use std/io.{File, Writer}
use std/io.File as StdFile
use ./parser.Parser
use ../shared/path.Path
pub use ./parser.Parser
pub use std/string.String
```

Meaning:

- `use path` introduces the module namespace under the path's final segment.
- `use path as Alias` introduces the module namespace under an explicit alias.
- `use path.Name` introduces one exported type-namespace name.
- `use path.Name as Alias` introduces one exported type-namespace name under an alias.
- `use path.{A, B}` introduces several exported type-namespace names; each may use `as`.
- `pub use path` re-exports a module namespace under its final segment.
- `pub use path.Name` and `pub use path.{...}` re-export selected public type names.

The default namespace name is the final path segment. `use std/io` introduces `io` and
`use ./path/to/parser` introduces `parser`. An alias is required when that name collides with an
existing visible name. `use / as root` is the explicit spelling for a package-root namespace,
because `/` has no final path segment.

Namespace members use `.` after import. The same selection shape works in value and type position;
resolution keeps those namespaces distinct:

```nct
use ./parser

func parse_text(text: &str): parser.Value! {
    return parser.parse(text)
}
```

Selected imports accept only names in the type namespace: built-in or nominal types, type aliases,
and interfaces. Functions, constants, and other values remain owned by their module namespace:

```nct
use std/io.File
use std/io

func write_message(file: &+File): void! {
    file.write_text("ready\n")?
    return
}
```

`use std/io.write` is an error. This keeps the subject of an external value operation visible at
every call site and prevents unrelated modules from flattening values into one local namespace.
Selected-name import braces are comma-delimited. One trailing comma is valid on either a single
line or multiple lines under [Comma-Delimited Lists](13-lexical-grammar.md#comma-delimited-lists).

Top-level imports precede non-import declarations. Block-scope module imports precede executable
statements in their block:

```nct
func greet(debug: bool): void {
    if debug {
        use std/io

        io.print("debug mode")
    }
}
```

A block import is a compile-time dependency even when its block is not executed. It cannot use
`pub`. Its bindings apply uniformly to value expressions, type positions, and constant
subexpressions inside body type annotations. Imports cannot shadow or collide with another visible
name; aliases resolve collisions.

Unsupported forms include wildcard imports, dotted module paths, explicit `.nct` suffixes, and
namespace alias re-exports:

```nct
use std/io.*
use std/io.print
use std.io.File
use ./config.nct.Config
pub use std/io as console
```

Module paths are valid only in `use`. An expression cannot call `std/io.print()` or
`./parser.parse()` directly. `use` always resolves a directory module; it never probes for or
selects an ordinary `.nct` source.

## Source Visibility

`see` makes declarations authored in one physical source directly visible from another physical
source. It does not load the target, add it to a module, or create a namespace.

```nct
// index.nct
see ./search.nct

pub func contains(text: &str, needle: &str): bool {
    return find(text, needle)
}
```

```nct
// search.nct
func find(text: &str, needle: &str): bool {
    ...
}
```

Here `search.nct` does not introduce a `search` namespace. `index.nct` may use `find` exactly as if
it were a private declaration written in `index.nct`, but this visibility does not spread to any
third source.

A `see` declaration has the following closed form and behavior:

- it is a private top-level declaration and cannot use `pub`, a selection, an alias, or block scope;
- its path begins with `./` or one or more `../` components, resolves relative to the authored
  source, and ends with the complete `.nct` filename;
- package-absolute paths, dependency aliases, module paths, directories, omitted extensions, and
  normalized `./../` forms are invalid;
- the canonical target must belong to the same physical directory module as the authored source;
  it cannot cross into a child module, parent module, another package, or nested package;
- it exposes only declarations authored in the exact target source;
- declarations visible to the target through its own `see`, `use`, lexical scopes, or synthetic
  prelude are not re-exposed;
- visibility is directional: if `a.nct` sees `b.nct`, `b.nct` does not see declarations from
  `a.nct` unless it writes its own reciprocal `see`;
- cycles are valid and idempotent because direct visibility is a set relation, not recursive source
  loading.

For example, direct-only visibility requires `a.nct` to name every source whose authored
declarations it uses:

```nct
// a.nct
see ./b.nct

func a(): i32 {
    return b() + c() // error: c is not visible
}
```

```nct
// b.nct
see ./c.nct

func b(): i32 {
    return c()
}
```

The fix is `see ./c.nct` in `a.nct`. The compiler does not compute a transitive source
namespace.

## Public Contracts and Private Definitions

`index.nct` is a readable module contract. A public callable may omit its body when one directly
seen source supplies its private definition:

```nct
// index.nct
see ./parse.nct

pub func parse(text: &str): Value!

instance Value {
    pub method &self.render(): String
}
```

```nct
// parse.nct
see ./index.nct

func parse(text: &str): Value! {
    ...
}

instance Value {
    method &self.render(): String {
        ...
    }
}
```

The private declaration completes the visible public declaration; it does not define a second
callable. The contract source must directly see the definition source, and the definition source
must directly see that `index.nct`; one-way or transitive visibility cannot form a
contract/definition pair.
The compiler joins the pair by module identity, declaration kind, owner, name, generic parameters
and bounds, receiver, parameter names and types, result type, authored `from` clause, and every
kind-specific contract modifier. These parts must have identical canonical source notation.
Visibility is written only on the public contract. Missing, mismatched, and duplicate definitions
are errors independent of source traversal order.

This rule applies to top-level functions, inherent methods, construction functions, typed literals,
coercion entries, and source-defined operators. An interface implementation is a bodyless
`impl Interface` member of an `instance` in `index.nct`; it does not participate in contract/body
joining. Private sources provide its required behavior through ordinary inherent method bodies.
The interface is the sole source of required signatures, so the root does not repeat a private
implementation method unless that method is also part of the public inherent API. Interface
requirements remain intrinsically bodyless.
Interface defaults write `default` explicitly and may use the same split: a bodyless
`pub default method` in the root interface is completed by one private `default method` body in a
reciprocally visible implementation interface fragment. `drop` always has a body and does not
participate in contract/body joining.

A `drop` declaration has no separately callable public contract. In a contract-first directory
module its mandatory body belongs to a private implementation source and is omitted from
`index.nct`; the type's ownership semantics expose destruction behavior without exporting that
body as an API entry.

```nct
// index.nct
see ./iterator_defaults.nct

pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
    pub default method self.count(): usize
}
```

```nct
// iterator_defaults.nct
see ./index.nct

interface Iterator {
    default method self.count(): usize {
        var source = move self
        var total: usize = 0
        while true {
            source.next() otherwise { return total }
            total = total + 1
        }
    }
}
```

The private interface fragment may complete only contracted default methods. It cannot declare
associated types, requirements, or a new interface surface.

Calls, imports, hover, completion, signature help, definition, and public diagnostics use the
contract in `index.nct`. Body checking and body diagnostics retain the implementation source.
Definition navigation selects the contract; implementation navigation selects the body. References
and rename treat both declarations and all uses as one semantic callable.

### Opaque Nominal Contracts

A public struct may omit its representation in `index.nct`:

```nct
// index.nct
see ./string.nct

pub struct String

construct String {
    /// Copies a static string view into owned storage.
    pub literal ""(text: &str): Self
    pub func empty(): Self
}

instance String {
    /// Exposes the initialized UTF-8 prefix without transferring ownership.
    pub coerce &self as &str
}
```

One directly seen source completes the representation and callable bodies:

```nct
// string.nct
see ./index.nct

struct String {
    storage: RawBuffer
    len: usize
}

construct String {
    literal ""(text: &str): Self {
        return String.copy(text)
    }

    func empty(): Self {
        return String {
            storage: empty_page_buffer(1),
            len: 0,
        }
    }
}

instance String {
    coerce &self as &str {
        return view(self)
    }
}
```

`pub struct String` is an opaque public nominal contract, not a fieldless struct. `struct String
{ ... }` completes that same nominal identity and owns its private representation. It cannot carry
visibility. A bodyless public nominal contract must have exactly one complete private definition;
an inline braced declaration already owns its representation and cannot be completed again.

An opaque contract exposes no fields or enum variants. A type that intentionally exposes fields or
variants writes its complete braced declaration in `index.nct`. A private nominal type may be
declared and defined in any reached source without a separate contract. `copy` is an observable
ownership contract: a separated copyable struct repeats `copy` on both its public contract and its
private definition, and the definition must satisfy the ordinary structural copy rules.

Only declarations authored in `index.nct` can define the module's exported namespace. An ordinary
source made visible with `see` cannot add a public name, member, construction entry, coercion,
operator, or interface implementation to that namespace. It may define private helpers and may complete
declarations already contracted in `index.nct`. Thus documentation, hover, completion, signature
help, and ordinary source review can derive the complete public use surface without reading
implementation sources.

## Re-exports

A public re-export can expose a child module namespace or selected public type names:

```nct
pub use ./parser
pub use ./parser.Parser
pub use std/io.File as StdFile
```

Rules:

- re-exports are allowed only in a module root source
- a namespace re-export does not flatten the target module
- a re-export boundary must be contained by the target name's boundary and can never widen it
- re-exported names participate in ordinary collision checks
- wildcard and namespace-alias re-exports are invalid
- selected-name re-exports do not also create a namespace alias
- functions, constants, and other values cannot be selected or re-exported without their module
  namespace

## Synthetic Standard Prelude

Every eligible user module receives a compiler-managed prelude from
`<Nocter-home>/std/prelude/index.nct`. The compiler does not rewrite source text or synthesize a
visible source-level import.

Rules:

- the prelude is applied to every user directory module
- every reached source in that module receives the same prelude fallback independently
- files inside the active Nocter home do not receive the synthetic prelude
- `std/prelude` itself does not receive the prelude
- a project path cannot shadow the compiler-selected prelude
- source-level `use std/prelude` and selected prelude imports are invalid
- prelude exports are fallback names: an explicit module declaration or import with the same local
  name takes precedence
- parameters, local bindings, and block imports likewise take precedence over a prelude name
- two authored names in the same scope remain an ordinary collision; fallback priority applies
  only to synthetic prelude exports
- project-wide prelude configuration is not supported

The standard prelude exports:

```nct
pub use std/string.String
pub use std/vec.Vec
pub use std/iter.Iterator
pub use std/map.Map
pub use std/set.Set
```

Named builtins such as `str` and primitive numeric types come from the compiler-managed universal
declaration fallback, while structural forms such as `[T]` come from the type grammar. Neither is
a prelude export. `Format`, `ExactSizeIterator`, file APIs, allocation APIs, process APIs, and
I/O functions require explicit imports from their domain modules. `Hash` and `HashState` likewise
require an explicit `std/hash` import.

## Package Layout

A package root is a directory whose `index.nct` contains exactly one top-level `#package`
directive. The same file is both the package declaration source and the root module's root source.
There is no separate manifest file and no source-root concept.

```text
project/
    index.nct
    search.nct
    parser/
        index.nct
        lexer.nct
    tests/
        unit/
            index.nct
```

The root `index.nct` contains package documentation, a directive prefix, and ordinary root-module
code:

```nct
//! Example application package.

#package: {
    name: "example",
    version: "0.1.0",
}
#executable: {
    name: "example",
}
#test: {
    name: "unit",
    module: "./tests/unit",
}
use std/io
use ./parser.Parser

func main(): i32! {
    let parser = Parser.new()
    io.print("ready\n")?
    return 0
}
```

Package-root rules:

- file documentation precedes the package directive prefix
- that file documentation belongs to the package; the root module does not register a second copy
- `#package` is required and requires string fields `name` and `version`
- `#dependencies`, `#executable`, and `#test` follow in the same directive prefix
- every package directive precedes `see`, `use`, and ordinary declarations
- `#executable` is repeatable, requires `name`, and accepts an optional `module`
- an omitted executable `module` selects `.`
- `#test` is repeatable and requires both `name` and `module`
- target module paths are `.` or package-relative directory paths beginning with `./`
- `module: "."` selects the package root `index.nct`
- `module: "./tools/app"` selects `tools/app/index.nct`
- targets never select ordinary implementation sources
- module paths omit `.nct` and cannot escape the package or cross a nested package
- package directives are invalid outside the package root `index.nct`
- a descendant `index.nct` containing `#package` starts a nested package; one without `#package`
  starts a child module
- dependency source intent and generated exact-selection fields remain together in `#dependencies`

The compiler does not discover a package target by probing `main.nct` or another conventional
filename.

## Implicit Standard-Library Package

The active Nocter home contributes one immutable package at `<Nocter-home>/std`. Its root
`index.nct` contains `#package`; the package name and version must match the toolchain
installation. Every compilation graph binds reserved dependency alias `std` to this exact package,
including imports written inside `std` itself.

User `#dependencies` must not contain `std`. A package named `std`, a directory with that spelling,
or a dependency alias cannot shadow the compiler-selected package or gain its primitive authority.
Single-file mode uses the same toolchain package without creating a package declaration for the
source file.

## Compile Units

Package `build`, `run`, and `check` begin with resolved target modules. Explicit file mode remains
available for isolated scripts and diagnostics as specified by
[Command Line Interface](15-command-line-interface.md).

A compile unit contains every physical source owned by each selected module, plus every module
reached recursively through `use` or the synthetic prelude. `see` contributes direct visibility
between already inventoried sources; it does not expand the compile unit. Physical sources are
loaded by canonical path at most once.

Rules:

- see cycles are valid
- module import and re-export cycles are errors
- executable entry lookup selects top-level `main` in the selected directory module, not an
  imported module
- the complete unit is resolved, type-checked, ownership-checked, and lowered as one program
- separate compilation, cached module artifacts, and link-time composition are not supported

## Source and Module Identity

Module identity is the exact package identity plus normalized module-directory path. Physical
source identity is a canonical absolute path.

Canonical source paths are used for loading, duplicate suppression, dependency invalidation, and
editor document mapping. Diagnostics retain a human display path and an optional canonical absolute
path. A declaration's definition location remains its physical source even though lookup uses the
shared directory-module namespace.

Example diagnostic paths:

```text
cwd:          /Users/me/project
source:       /Users/me/project/parser/lexer.nct
display:      parser/lexer.nct
absolute:     /Users/me/project/parser/lexer.nct
```

```text
Nocter home:  /Users/me/.nocter
source:       /Users/me/.nocter/std/io/index.nct
display:      std/io/index.nct
absolute:     /Users/me/.nocter/std/io/index.nct
```

## Path Resolution

A `see` path begins with `./` or repeated `../`, resolves from the authored source's directory, and
names exactly one existing `.nct` file. It does not probe an extensionless alternative or a
directory module. Canonical resolution must keep both sources in the same module.

Relative module imports begin with `./` or `../`, resolve from the importing source's directory,
and select only a directory containing `index.nct`. A module path omits both `index.nct` and the
`.nct` extension.

Package-absolute paths begin with `/` and resolve directory modules from the owning package root:

```nct
use /parser.Parser
use /.RootValue
```

The second form selects a type name directly from the package root module. Bare `use /` is invalid
because a namespace import requires a local name; write `use / as root` instead.

Non-relative paths begin with a declared dependency alias or `std` and resolve directory modules
only:

```nct
use json/value.Value
use std/io
```

Rules:

- relative module paths cannot leave their package or select an ordinary source file
- a leading `/` is package-absolute, never filesystem-absolute
- `use config.Config` requires a dependency alias named `config`; it does not search project files
- `std` is a reserved implicit dependency bound to the active toolchain standard-library package
- packages must not declare or lock a dependency named `std`
- `.nct` is required in `see` and omitted from `use`
- `index.nct` is the only directory-module root convention
- Nocter home comes from `NOCTER_HOME` when set, otherwise from the real running compiler path

## Name Resolution

Every physical source has its own authored source namespace. Unqualified lookup from that source
uses:

1. current and enclosing lexical bindings
2. function parameters
3. declarations authored in the current source
4. declarations authored in sources named by a direct `see`
5. explicit module imports authored in the current source and synthetic prelude names
6. built-in types and syntax forms

Shadowing authored names is not supported. Parameters, locals, block imports, module declarations,
authored imports, built-in type names, and the contextual `Self` type form must not introduce the
same visible name. The synthetic prelude is the sole exception: it is a fallback layer, so any
authored source name or valid lexical name with the same spelling takes precedence. Two private
declarations with the same spelling may exist in different sources when no one source sees both.
If one source sees both through its direct `see` set, lookup reports an authored-name collision;
source traversal order never selects one.

## Visibility

Definitions are private by default. A `pub(...)` scope exposes a name to a selected ancestor module
tree or to its package. Bare `pub` exposes a name to every package.

```nct
// std/io/index.nct
pub struct File {
    fd: i32
}

construct File {
    pub func open(path: &str): Self! {
        ...
    }
}

pub func stdout(): File {
    ...
}
```

```nct
// std/ptr/index.nct
pub(/) primitive func from_addr<T>(address: usize): *T
```

Rules:

- public declarations may be written only in `index.nct`
- top-level types, aliases, interfaces, functions, primitives, fields, methods, interface members,
  construction entries, coercion entries, and re-exports follow this
  rule
- an implementation definition that completes an `index.nct` contract omits visibility
- private declarations are visible only in their authored source and in sources that directly see it
  directly
- `pub(./)` exposes the declaring module and all descendant modules
- each `../` in `pub(../)`, `pub(../../)`, and deeper forms moves the boundary to one ancestor
  module; the boundary cannot move above the package root
- `pub(/)` exposes every module in the declaring package
- bare `pub` exposes every package
- scoped visibility is interpreted from the declaring directory module
- names, dependency aliases, and arbitrary module paths are not valid inside `pub(...)`
- a re-export may narrow a boundary but cannot widen it
- variants declared inline in `index.nct` follow their enum's visibility; variants supplied by a
  private representation definition remain private to their authored source and direct seers
- `instance` declarations and their `impl Interface` members are not themselves marked public
- there is no `private` keyword, friend namespace, or named visibility scope

Visibility grants source access only. The exact implicit `std` package identity separately grants
authority to declare registered primitives and provide compiler-owned runtime roles. Writing
`pub(/)` in an ordinary package never grants that authority.
