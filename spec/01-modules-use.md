# Modules and Use Declarations

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Directory Modules

Nocter has no `module` declaration. A directory containing `index.nct` defines one module, and the
directory path defines that module's identity.

```text
project/
    nocter.nct             package declarations
    index.nct              root module root source
    string.nct             root module source
    parser/
        index.nct          child module `parser`
        lexer.nct          source of module `parser`
    internal/
        scanner.nct        source folder; not a module
```

`index.nct` is the module root source file. It defines the module's public surface. Other `.nct`
files may contribute private declarations when reached through source imports. Their basenames do
not create namespaces. A directory without `index.nct` is a source folder.

Import paths use `/` and omit `.nct`:

```text
/work/project/index.nct                    => /
/work/project/parser/index.nct             => /parser
~/.nocter/std/io/index.nct                  => std/io
```

## Module Imports

`use` is lexical compile-time syntax, not a runtime statement. Module imports may introduce a
namespace, selected public names, or public re-exports:

```nct
use std/io
use std/io.{File, stdout, stderr}
use std/io.File as StdFile
use ./parser.Parser
use ../shared/path.Path
pub use ./parser.Parser
pub use std/string.String
```

Meaning:

- `use path` introduces the module namespace under the path's final segment.
- `use path.Name` introduces one exported name.
- `use path.Name as Alias` introduces one exported name under an alias.
- `use path.{A, B}` introduces several exported names; each may use `as`.
- `pub use path` re-exports a module namespace under its final segment.
- `pub use path.Name` and `pub use path.{...}` re-export selected public names.

The default namespace name is the final path segment. `use std/io` introduces `io` and
`use ./path/to/parser` introduces `parser`.

Top-level imports precede non-import declarations. Block-scope module imports precede executable
statements in their block:

```nct
func greet(debug: bool): void {
    if debug {
        use std/io.print

        print("debug mode")
    }
}
```

A block import is a compile-time dependency even when its block is not executed. It cannot use
`pub`. Imports cannot shadow or collide with another visible name; aliases resolve collisions.

Unsupported forms include wildcard imports, dotted module paths, explicit `.nct` suffixes,
namespace alias re-exports, and textual inclusion:

```nct
use std/io.*
use std.io.File
use ./config.nct.Config
pub use std/io as console
include ./search
```

Paths are valid only in `use`. An expression cannot call `std/io.print()` or
`./parser.parse()` directly.

## Same-Module Source Imports

A private top-level bare relative import may compose another physical source file into the current
module:

```nct
// index.nct
use ./search

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

Here `search.nct` does not introduce `search`, and `find` is visible throughout the composed
module. Source import rules are deliberately narrower than module imports:

- the path must resolve a `.nct` file whose nearest enclosing module root is the same `index.nct`
- the declaration must be exactly a private top-level `use ./path` or `use ../path`
- selected names, aliases, block scope, and `pub use` are invalid for source imports
- imported sources may import other sources in the same module
- source cycles are allowed and idempotent; declarations are collected before bodies are resolved
- a source file not reachable from the module root source is not compiled
- a source file and child directory module cannot occupy the same logical path

For example, if both `search.nct` and `search/index.nct` exist, `use ./search` is ambiguous. Rename
one side; the compiler does not choose by precedence.

Only a module root source may contain non-private declarations, fields, interface members,
construction or coercion entries, or re-exports. Implementation sources are private parts of the
module. This keeps every module boundary readable in `index.nct`.

## Public Callable Contracts and Bodies

A public callable in `index.nct` may omit its body when one explicitly imported source of the same
module supplies the body:

```nct
// index.nct
use ./parse

pub func parse(text: &str): Value!

instance Value {
    pub method &self.render(): String
}
```

```nct
// parse.nct
func parse(text: &str): Value! {
    ...
}

instance Value {
    method &self.render(): String {
        ...
    }
}
```

The body declaration is private and does not define a second callable. The compiler joins it to
the public contract by directory-module identity, callable kind, owner, name, generic parameters
and bounds, receiver, parameter names and types, result type, and authored `from` clause. These
parts must have identical canonical source notation. Missing, mismatched, and duplicate bodies are
errors independent of source traversal order.

This rule applies to top-level and associated functions, inherent methods, construction functions,
typed literals, and coercion entries. A construction implementation does not repeat `default`.
Interface requirements and conformance methods keep their conformance model;
interface default methods remain inline, and `drop` always has an inline body.

Calls, imports, hover, completion, signature help, definition, and public diagnostics use the
contract in `index.nct`. Body checking and body diagnostics retain the implementation source.
Definition navigation selects the contract; implementation navigation selects the body. References
and rename treat both declarations and all uses as one semantic callable.

## Re-exports

A public re-export can expose a child module namespace or selected public names:

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

## Synthetic Standard Prelude

Every eligible user module receives a compiler-managed prelude from
`<Nocter-home>/std/prelude/index.nct`. The compiler does not rewrite source text or synthesize a
visible source-level import.

Rules:

- the prelude is applied to every user directory module
- all physical sources in one module share the same module namespace and prelude surface
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
pub use std/iter.{Iterable, IntoIterator, Iterator}
```

Built-in forms such as `str`, `[T]`, and primitive numeric types are language types, not prelude
exports. `Format`, `Sequence`, `ExactSizeIterator`, file APIs, allocation APIs, process APIs, and
I/O functions require explicit imports from their domain modules.

## Package Layout

A package root is a directory that directly contains `nocter.nct`. There is no source-root
concept.

```text
project/
    nocter.nct
    index.nct
    search.nct
    parser/
        index.nct
        lexer.nct
    tests/
        unit/
            index.nct
```

`nocter.nct` contains package documentation and directives only:

```nct
//! Example application package.

#name: "example"
#version: "0.1.0"
#executable: {
    name: "example",
}
#test: {
    name: "unit",
    module: "./tests/unit",
}
```

The root `index.nct` contains ordinary Nocter code:

```nct
use std/io.print
use ./parser.Parser

func main(): i32! {
    let parser = Parser.new()
    print("ready\n")?
    return 0
}
```

Package-file rules:

- file documentation precedes package directives; ordinary code is rejected in `nocter.nct`
- `#name` defaults to the package-directory basename for display only
- `#version` remains absent when omitted
- `#executable` is repeatable, requires `name`, and accepts an optional `module`
- an omitted executable `module` selects `.`
- `#test` is repeatable and requires both `name` and `module`
- target module paths are `.` or package-relative directory paths beginning with `./`
- `module: "."` selects the package root `index.nct`
- `module: "./tools/app"` selects `tools/app/index.nct`
- targets never select ordinary implementation sources
- module paths omit `.nct` and cannot escape the package or cross a nested package
- package directives are invalid outside `nocter.nct`
- a nested `nocter.nct` starts another package; a nested `index.nct` starts a child module
- dependency declarations and generated exact locks remain in `nocter.nct`

The compiler does not discover a package target by probing `main.nct` or another conventional
filename.

## Implicit Standard-Library Package

The active Nocter home contributes one immutable package at `<Nocter-home>/std`. It contains its
own `nocter.nct` and root `index.nct`; the package name and version must match the toolchain
installation. Every compilation graph binds reserved dependency alias `std` to this exact package,
including imports written inside `std` itself.

User `#dependencies` and generated `#lock` data must not contain `std`. A package named `std`, a
directory with that spelling, or a dependency alias cannot shadow the compiler-selected package or
gain its primitive authority. Single-file mode uses the same toolchain package without creating a
manifest for the source file.

## Compile Units

Package `build`, `run`, and `check` begin with resolved target modules. Explicit file mode remains
available for isolated scripts and diagnostics as specified by
[Command Line Interface](15-command-line-interface.md).

A compile unit contains each selected module and every module or source reached recursively through
imports and the synthetic prelude. Physical sources are loaded by canonical path at most once.

Rules:

- source-import cycles within one module are valid
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

## Import Path Resolution

Relative imports begin with `./` or `../` and resolve from the importing source's directory.
Relative resolution considers both a same-module source (`path.nct`) and a child module
(`path/index.nct`); finding both is an ambiguity error.

Package-absolute paths begin with `/` and resolve directory modules from the owning package root:

```nct
use /parser.Parser
```

Non-relative paths begin with a declared dependency alias or `std` and resolve directory modules
only:

```nct
use json/value.Value
use std/io.print
```

Rules:

- relative paths cannot leave their package or enter another module's implementation source
- a leading `/` is package-absolute, never filesystem-absolute
- `use config.Config` requires a dependency alias named `config`; it does not search project files
- `std` is a reserved implicit dependency bound to the active toolchain standard-library package
- packages must not declare or lock a dependency named `std`
- `.nct` is omitted from imports
- `index.nct` is the only directory-module root convention
- Nocter home comes from `NOCTER_HOME` when set, otherwise from the real running compiler path

## Name Resolution

Unqualified lookup uses the shared module namespace plus lexical scopes:

1. current and enclosing lexical bindings
2. function parameters
3. declarations in any composed source of the current module
4. explicit imported names and synthetic prelude names
5. built-in types and syntax forms

Shadowing is not supported. Parameters, locals, module declarations, imports, prelude names, and
built-in type names must not introduce the same visible name. Duplicate top-level declarations are
diagnosed across every source in the module, independent of source traversal order.

## Visibility

Definitions are private by default. A `pub(...)` scope exposes a name to a selected ancestor module
tree or to its package. Bare `pub` exposes a name to every package.

```nct
// std/io/index.nct
pub struct File {
    fd: i32
}

construct File {
    pub default func open(path: &str): Self! {
        ...
    }
}

pub func stdout(): File {
    ...
}
```

```nct
// std/ptr/index.nct
pub(/) primitive from_addr<T>(address: usize): *T
```

Rules:

- public declarations may be written only in `index.nct`
- top-level types, aliases, interfaces, functions, primitives, fields, associated functions,
  methods, interface members, construction entries, coercion entries, and re-exports follow this
  rule
- private declarations in every composed source are visible throughout their module
- `pub(./)` exposes the declaring module and all descendant modules
- each `../` in `pub(../)`, `pub(../../)`, and deeper forms moves the boundary to one ancestor
  module; the boundary cannot move above the package root
- `pub(/)` exposes every module in the declaring package
- bare `pub` exposes every package
- scoped visibility is interpreted from the declaring directory module, so all implementation
  sources in that module share one boundary
- names, dependency aliases, and arbitrary module paths are not valid inside `pub(...)`
- a re-export may narrow a boundary but cannot widen it
- enum variants follow their enum's visibility
- `instance` and `conform` declarations are not themselves marked public
- there is no `private` keyword, friend namespace, or named visibility scope

Visibility grants source access only. The exact implicit `std` package identity separately grants
authority to declare registered primitives and provide compiler-owned runtime roles. Writing
`pub(/)` in an ordinary package never grants that authority.
