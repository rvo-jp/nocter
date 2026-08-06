# Modules and Use Declarations

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Modules

Adopted: Nocter modules are derived from file paths. The language does not have a `module` declaration.

A module identity is derived from the canonical source file path. One `.nct` file defines one module.

Import paths use `/` as the path separator:

```text
examples/word_count.nct                                  => examples/word_count
~/.nocter/std/io.nct                                     => std/io
~/.nocter/std/os.nct                                     => std/os
```

The file path is the source of truth. There is no separate module name inside the file.

`use` declarations make names from another module available. A `use`
declaration is lexical compile-time syntax, not a runtime statement.

```nct
use std/io.{File, stdout}
use std/mem.Allocator
```

Adopted `use` forms:

```nct
use std/mem.Allocator
use std/io.{File, stdout, stderr}
use std/io.File as StdFile
use std/io
use ./config.AppConfig
use ./config
use ../shared/path.Path
pub use std/string.String

use std/io as console
```

Top-level `use` declarations must appear at the start of the source file before
non-`use` declarations. Block-scope `use` declarations must appear at the start
of a `{ ... }` block before executable statements, bindings, or result
expressions:

```nct
func greet(): void {
    use std/io.print

    print("hello")
}
```

The scope of a block-scope `use` starts after the declaration and ends at the
end of that block. Nested blocks may use their own block-scope imports:

```nct
func process(debug: bool): void {
    if debug {
        use std/io.print

        print("debug mode")
    }

    print("done")
    // error: print is not visible outside the if block
}
```

`pub use` is allowed only at the top level. A block-scope `use` cannot re-export
anything.

Even inside `if`, `match`, `while`, or `loop`, a block-scope `use` is not a
conditional dependency. The imported module is loaded as part of the compile
unit whenever the containing file is compiled.

Meaning:

- `use path` imports the module namespace under its default name.
- `use path.Name` imports one exported name into the current file.
- `use path.Name as Alias` imports one exported name under an alias.
- `use path.{Name}` is accepted as the braced single-name spelling.
- `use path.{NameA, NameB}` imports multiple exported names from one file.
- Each imported item in a braced `use` list may independently use `as Alias`.
- `pub use path.Name` or `pub use path.{Name}` imports and re-exports one public name.
- `use path as alias` imports the module namespace under an alias.
- The default namespace name is the final non-relative segment of the module path. `use std/io` introduces `io`; `use ./path/to/dir` introduces `dir`, even when the path resolves to `./path/to/dir/index.nct`.

Examples:

```nct
use std/io
use std/io as console
use std/io.File as StdFile

var out = io.stdout()
var err = console.stderr()
let file: StdFile = StdFile.open(path)?
```

Relative and absolute module path prefixes are valid only inside `use`
declarations. Code must not call a module by writing a relative or absolute
path-like expression:

```nct
./path/to/file.something()
../shared/file.something()
/absolute/path/file.something()
// invalid: import the module namespace first, then call file.something()
```

This rule does not give special expression meaning to non-relative module paths.
Outside `use`, text such as `std/io.print("hello")` is parsed as ordinary
expression tokens, the same as `std / io.print("hello")`. If `std` and `io` are
not ordinary visible values, normal name resolution fails.

`use std/prelude` is not a source-level import form. User project modules receive the standard prelude synthetically as described in [Synthetic Standard Prelude](#synthetic-standard-prelude).

```nct
use std/prelude
// invalid: the prelude is compiler-managed
```

Name collisions are compile errors.

```nct
use std/io.File
use ./my/fs.File
// error: File is imported twice
```

Use aliases to resolve collisions.

```nct
use std/io.File as StdFile
use ./my/fs.File as MyFile
```

Block-scope imports follow the same collision rule. They must not shadow an
outer visible name, a parameter, a local binding, another import, a top-level
declaration, a prelude name, or a built-in type name:

```nct
use std/io.print

func debug(): void {
    use debug/console.print
    // error: print is already visible; write `as debug_print`
}
```

Not adopted:

```nct
import std/io
use std/io.*
pub use std/io.*
import std/io.File
pub use std/io as io
pub use std/io
use std/prelude
use std/prelude.Error
use ./config.nct.Config
include std/prelude
./path/to/file.something()
```

Wildcard imports, bare public re-exports, dotted module paths, namespace alias re-exports, source-level prelude imports, explicit `.nct` extensions in import paths, textual include, and relative or absolute path-like module expressions are not part of the initial language.

## Re-exports

Adopted: public re-export may expose a module namespace or selected public names.

```nct
pub use ./src/json
pub use std/string.String
pub use std/io.File as StdFile
```

`pub use path.Name` and `pub use path.{Name}` mean:

- load the module at `path`
- import the public name `Name` into the current module
- expose that imported name as part of the current module's public API

Rules:

- `pub use path` re-exports the resolved module namespace under the final path segment.
- A namespace re-export does not flatten declarations from the target module.
- `pub use path.Name` and `pub use path.{...}` are allowed only at top level.
- `pub use path.Name` and `pub use path.{...}` can re-export only public names from the source module.
- `pub(nocter)` names are not public names for `pub use path.Name` or `pub use path.{...}`.
- Each item in a `pub use` list may independently use `as Alias`.
- Re-exported names participate in the same name collision checks as other imports and top-level declarations.
- `pub use path.*` is invalid.
- `pub use path as alias` remains invalid; namespace re-exports use their final path segment.
- `pub use path.Name` and `pub use path.{...}` do not make private names public.
- Selected-name re-exports do not create a namespace alias.
- Import cycles involving `pub use path.Name` or `pub use path.{...}` are still import cycles and are errors in the initial design.

## Synthetic Standard Prelude

Adopted: user project modules receive a compiler-managed synthetic standard prelude loaded from `std/prelude.nct` in the active Nocter home.

The compiler does not rewrite source text and does not model the prelude as a source-level `use std/prelude` item. Diagnostics, formatting, AST source spans, and editor views should continue to refer to the user's original source.

The purpose is to avoid requiring this boilerplate in every file while keeping prelude behavior defined as an import rule rather than as special compiler treatment for ordinary standard-library names. Built-in forms such as `str`, `&str`, `[T]`, `&[T]`, and `&+[T]` are not provided by the prelude.

Initial rules:

- Every user project module receives a synthetic file-local prelude import from `std/prelude.nct`.
- The synthetic prelude is applied independently to each user project module.
- The synthetic prelude does not propagate from one file to another; each user project file gets its own synthetic prelude.
- The synthetic prelude is not applied to files inside the active Nocter home.
- The synthetic prelude is not applied to common standard-library files under `std/`.
- The synthetic prelude is not applied to `std/prelude.nct` itself.
- The synthetic prelude path is resolved directly under the active Nocter home;
  a user project file such as `std/prelude.nct` does not shadow it.
- A source-level `use std/prelude`, `use std/prelude.Name`, `use std/prelude.{...}`, or `use std/prelude as name` is invalid in v0.2.0.
- The prelude imports all public exported names from `std/prelude.nct` into the current file.
- Source-level `use path` does not import every public exported name from `path`; it imports only the module namespace.
- `include std/prelude` is invalid.
- Names introduced by the synthetic prelude participate in the same collision checks as explicit imports.
- If a prelude name collides with a local declaration, top-level declaration, parameter, local binding, explicit import, or built-in name, the program is invalid.
- Diagnostics should identify collisions with the synthetic prelude as prelude collisions, not as hidden compiler built-ins.
- Project-wide prelude configuration is not part of the initial design.

Initial prelude surface direction:

```nct
pub use std/error.{Error, ErrorCode}
pub use std/string.String
```

The prelude must remain small. `Int` is not part of v0.2.0; write `i32` or define a project-local alias. `Vec` is not re-exported by the prelude in v0.2.0 because collections remain an explicit domain module surface. Names such as `Vec`, `File`, `Allocator`, `Layout`, `RawBuffer`, `print`, `stdout`, `stderr`, `args`, `env`, `cwd`, `exit`, and `abort` should be imported explicitly from their domain modules.

## Package Layout

Adopted for v0.4.0 Phase 1: a package root is a directory that directly contains `nocter.nct`.
There is no source-root concept. `index.nct` remains only a directory module.

```text
project/
    nocter.nct
    src/
        app.nct
        config.nct
```

```nct
//! Example application package.

#name: "example"
#version: "0.1.0"
#executable: {
    name: "example",
    entry: "./src/app",
}

pub use ./src/config
```

Package-file rules:

- File documentation precedes package directives; ordinary imports and declarations follow them.
- `#name` and `#version` accept one string and may occur at most once.
- `#name` defaults to the root directory basename for display only. That display name is not
  package identity.
- An omitted `#version` remains absent.
- `#executable` is repeatable, requires a `name` string, and accepts an optional `entry` string.
- Omitting `entry` selects the package-root module in `nocter.nct`.
- `#test` is repeatable and requires both a `name` string and an `entry` string. Test names are
  unique among tests; executable and test names occupy separate typed target namespaces.
- Test entries use the same exact logical-module resolution and package-containment rules as
  executable entries. The compiler never discovers tests by scanning a directory.
- `entry: "."` selects the root directory module at `index.nct`.
- `entry: "./src/app"` resolves `src/app.nct` or `src/app/index.nct`.
- Logical module paths omit `.nct`, cannot escape the package root lexically or through symbolic
  links, and are ambiguous when both file and directory-module forms exist.
- Neither the package-root `nocter.nct` nor an explicit target entry may escape its package or
  cross into a nested package through a symbolic link or path.
- Package directives are invalid outside `nocter.nct`. A nested `nocter.nct` defines another
  package; a nested `index.nct` remains an ordinary directory module.
- Omitting a source from `build`, `run`, or `check` selects `./nocter.nct`; it never probes for
  `main.nct`.
- Explicit positional source files and `--file` retain single-file operation without changing the
  package command's default.
- Relative imports are resolved from the directory containing the importing file and cannot leave
  its package.
- Leading `/` is package-absolute, not filesystem-absolute.
- A non-relative first segment names a declared dependency or `std`.
- `#dependencies` declares path, Git, and archive sources. Generated format-1 `#lock` data fixes
  Git commits and archive SHA-256 content in the same `nocter.nct`.

Example:

```nct
// app.nct
use std/io.print
use /src/config.Config

func main(): i32! {
    let config = Config.default()
    print(config.name)?

    return 0
}
```

```nct
// src/config.nct
pub struct Config {
    pub name: &str
}

construct Config {
    pub default func default(): Self {
        return Config {
            name: "Nocter",
        }
    }
}
```

## Compile Unit

`nocter build app.nct`, `nocter run app.nct`, and `nocter check app.nct` treat `app.nct` as the root file. The CLI contract is specified in [Command Line Interface](15-command-line-interface.md).

The compile unit is the root file plus every `.nct` file reached by following
top-level `use`, block-scope `use`, public re-export, and eligible synthetic
prelude loads recursively.

Rules:

- Top-level `use` and `pub use` declarations are allowed only at the start of a
  source file. Block-scope `use` declarations are allowed only at the start of
  a block.
- `pub use` is allowed only at top level.
- The synthetic prelude load is compiler-internal and behaves as if its names are introduced before source-level imports for eligible user project modules.
- Top-level executable statements are not allowed.
- A root executable must define top-level `main` in the root file.
- Entry lookup does not select imported functions.
- Imported files may define ordinary functions named `main`, subject to normal name visibility and duplicate-name rules.
- The same canonical file path is loaded at most once, even if reached through different relative paths.
- Import cycles are errors in the initial design.
- The whole compile unit is name-resolved, type-checked, ownership-checked, and lowered as one program.
- Separate compilation, incremental compilation, cached module artifacts, and link-time composition of multiple Nocter compile units are not part of v0.2.0.

## Source File Identity

Adopted: compiler-internal source file identity is the canonical absolute path.

Rules:

- Every loaded source file has a canonical absolute path.
- Canonicalization resolves `.` and `..` path components.
- Canonicalization resolves symlinks when the host filesystem can report the real path.
- The import graph uses canonical absolute paths for duplicate detection.
- The same canonical absolute path is one source file, even if reached through multiple relative import paths.
- A symlink path and its real path refer to the same source file when canonicalization resolves them to the same path.
- Import cycles are detected using canonical absolute paths.
- Filesystem errors during canonicalization are reported as command-line, filesystem, or import diagnostics depending on which path triggered the failure.
- The language does not expose canonical file paths to Nocter source code.

Diagnostic display path rules:

- Diagnostics keep both a display path and a canonical absolute path.
- The display path is intended for humans.
- The canonical absolute path is intended for editor integrations, LSP document mapping, and compiler de-duplication.
- If a file is under the command working directory, the display path is relative to that working directory.
- If a file is under the common Nocter home `std/`, the display path starts with `std/`.
- Otherwise, the display path is the canonical absolute path.
- Display paths use `/` as the separator in diagnostics, even on future non-macOS hosts.

Examples:

```text
cwd:          /Users/me/project
source file:  /Users/me/project/src/parser.nct
display:      src/parser.nct
absolute:     /Users/me/project/src/parser.nct
```

```text
Nocter home:  /Users/me/.nocter
source file:  /Users/me/.nocter/std/io.nct
display:      std/io.nct
absolute:     /Users/me/.nocter/std/io.nct
```

## Import Path Resolution

Import paths select module-relative, package, dependency, or standard-library namespaces. The
`.nct` extension is omitted.

Relative import paths start with `./` or `../`.

```nct
use ./config.AppConfig
use ../shared/path.Path
```

Relative paths are resolved from the directory containing the current file:

```text
current file: app/main.nct
import path:  ./config
resolved:     app/config.nct
```

Package-absolute paths start with `/`.

```nct
use /src/config.Config
```

Non-relative paths start with a dependency alias or `std`.

```nct
use json/value.Value
use std/io.print
```

For a package at `/work/app` with dependency alias `json`, the namespaces are:

```text
/work/app/.nocter/packages/<json-PackageId>/nocter.nct
/work/app/.nocter/packages/<json-PackageId>/value.nct
/opt/nocter/std/io.nct
/opt/nocter/std/io/index.nct
```

Each module path first tries `path.nct`, then `path/index.nct`. If both exist in the same import root, the import is ambiguous and must be reported as an error.

```text
use /src/json.Parser
/work/app/src/json.nct
/work/app/src/json/index.nct
```

Rules:

- Package modules use `./`, `../`, or a leading `/`; relative paths cannot escape the package.
- `use config.Config` requires a dependency named `config`; it never searches project directories.
- `use std/io` resolves only through the compiler-matched Nocter home and cannot be shadowed by a
  project directory.
- `/src/path` is resolved from the owning package root. Filesystem-absolute imports are invalid.
- `.` is not a module separator in import paths.
- `.nct` is not written in import declarations.
- Directory modules use `index.nct`; `mod.nct` directory modules are not part of v0.2.0.
- The compiler locates Nocter home from `NOCTER_HOME` if set, otherwise from the resolved real path of the running `nocter` executable and its parent directory. This supports normal installs where a `PATH` directory contains a symlink to `~/.nocter/nocter`.
- The compiler does not automatically search `cwd/.nocter` or `~/.nocter`.
- The repository local release image `dist/.nocter/` may act as Nocter home during local development. This is a development detail, not the user-facing installation convention.

## Name Resolution

Unqualified names are resolved inside one file after imports are loaded and visibility is checked.

Lookup order:

1. Current lexical block bindings.
2. Outer lexical block bindings.
3. Function parameters.
4. Same-file top-level declarations.
5. Explicitly imported names and names introduced by the synthetic prelude.
6. Built-in types and reserved syntax forms.

Initial rules:

- Shadowing is not allowed.
- Function parameter names must be unique within the parameter list.
- A function parameter must not reuse a visible local, parameter, top-level, imported, prelude, or built-in type name.
- A local binding must not reuse a visible local, parameter, top-level, imported, or built-in type name.
- Two imports, a re-export, or a prelude name introducing the same local name are errors.
- A same-file top-level declaration and an imported name must not have the same local name.
- `use path` introduces only the default namespace name.
- `use path as alias` introduces only the explicit namespace alias name.
- Names inside an imported namespace are accessed with member syntax, such as `io.stdout()`.
- There is no wildcard import.
- There is no implicit import of every name from `std`.
- The synthetic prelude is limited to `std/prelude`; it is not a general implicit import facility.

## Visibility

Adopted: definitions are private by default. Public API is marked with `pub`. Nocter-distribution-internal API is marked with `pub(nocter)`.

```nct
pub struct File {
    fd: i32
}

construct File {
    pub default func open(path: &str): Self! {
        ...
    }
}

impl File {
    method &self.raw_fd(): i32 {
        return self.fd
    }
}

pub func stdout(): File {
    ...
}
```

```nct
pub(nocter) primitive from_addr<T>(address: usize): *T
```

Rules:

- Top-level definitions are private to their module by default.
- `pub` on a top-level definition makes it importable from other modules.
- `pub(nocter)` on a top-level definition makes it importable only from modules inside the active Nocter home.
- The active Nocter home includes the common `std/` tree.
- `pub(nocter)` is intended for distributed standard-library internals such as restricted pointer APIs and target primitive boundaries.
- `pub(nocter)` may be written only in modules inside the active Nocter home.
- `pub(nocter)` is not user-project package visibility.
- `nocter` is contextual inside the `pub(nocter)` modifier. It is not a globally reserved keyword.
- Type aliases are top-level definitions. `type Name = Target` is private by default, `pub type Name = Target` makes the alias importable and re-exportable, and `pub(nocter) type Name = Target` makes the alias importable only inside the active Nocter home.
- Interfaces are top-level definitions. `interface Name { ... }` is private by
  default, `pub interface Name { ... }` makes the contract importable and
  re-exportable, and `pub(nocter) interface Name { ... }` makes the contract
  importable only inside the active Nocter home.
- `use` can import `pub` names from any module.
- `use` can import `pub(nocter)` names only when the importing module is inside the active Nocter home.
- User project modules cannot import `pub(nocter)` names.
- `pub use path.Name` and `pub use path.{...}` can re-export only `pub` names from the source module as part of the current module's public API.
- `pub use path.Name` and `pub use path.{...}` cannot re-export `pub(nocter)` names as public API.
- `pub use path.Name` and `pub use path.{...}` re-export the imported name as part of the current module's public API.
- Struct fields are private by default.
- Public struct fields must be marked with `pub`.
- `pub(nocter)` struct fields are visible only to modules inside the active Nocter home.
- Functions and methods are private by default.
- Public associated functions declared as `func Type.name` and public methods inside `impl` blocks must be marked with `pub`.
- Nocter-distribution-internal associated functions and methods may be marked with `pub(nocter)`.
- `impl` blocks themselves are not marked `pub`.
- Enum variants follow the visibility of their enum in the initial design.
- Interface members must be explicitly marked `pub`.
- There is no `private` keyword in the initial design.
- There is no standalone `export` declaration in the initial design.
- `pub(package)`, `pub(crate)`, `pub(std)`, `pub(home)`, and `pub(trusted)` are not part of v0.2.0.

Example:

```nct
pub struct Point {
    pub x: i32
    pub y: i32
}

pub enum Direction {
    north
    south
    east
    west
}
```

Initial rules:

- One `.nct` file defines one module.
- The `.nct` extension is removed.
- File and directory names used for modules must be snake_case identifiers as defined by [Lexical Grammar](13-lexical-grammar.md#identifiers).
- `module` is not a keyword.
- Directory modules use `index.nct`.
- Standard library modules live under `std`.
- `/work/app/src/io.nct` is written `/src/io`; `std/io` always selects the standard library.
- Target-dependent standard-library declarations are selected by `#target: "..."` inside stable module files such as `~/.nocter/std/os.nct`; target names are not required in import paths.

Import namespaces:

1. The current file directory for `./` and `../` paths.
2. The owning package root for `/...` paths.
3. The dependency bound by the first path segment in `#dependencies`.
4. The active Nocter home only for `std/...`.

The compiler locates Nocter home in this order:

1. `NOCTER_HOME`, if set.
2. The directory containing the running `nocter` executable.
3. Otherwise, report a clear configuration error.
