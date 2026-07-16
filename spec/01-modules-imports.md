# Modules and Imports

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Modules

Adopted: Nocter modules are derived from file paths. The language does not have a `module` declaration.

A module identity is derived from the canonical source file path. One `.nct` file defines one module.

Import paths use `/` as the path separator:

```text
examples/word_count.nct                                  => examples/word_count
~/.nocter/std/io.nct                                     => std/io
~/.nocter/std/os/macos.nct                               => std/os/macos
```

The file path is the source of truth. There is no separate module name inside the file.

Imports make names from another module available.

```nct
from std/io import File, stdout
from std/mem import Allocator
```

Adopted import forms:

```nct
from std/mem import Allocator
from std/io import File, stdout, stderr
from std/io import File as StdFile
from ./config import AppConfig
from ../shared/path import Path
pub from std/string import String

import std/io as io
```

Meaning:

- `from path import Name` imports one exported name into the current file.
- `from path import NameA, NameB` imports multiple exported names from one file.
- `from path import Name as Alias` imports one exported name under an alias.
- Each imported item in a `from` list may independently use `as Alias`.
- `pub from path import Name` imports and re-exports one public name.
- `import path as alias` imports the module namespace under an alias.

Examples:

```nct
import std/io as io
from std/io import File as StdFile

var out = io.stdout()
let file = StdFile.open(path)?
```

`use std/prelude` is not normally written in user source. User project modules receive the standard prelude synthetically as described in [Synthetic Standard Prelude](#synthetic-standard-prelude).

```nct
use std/prelude
// accepted but redundant in a user project module
```

Name collisions are compile errors.

```nct
from std/io import File
from ./my/fs import File
// error: File is imported twice
```

Use aliases to resolve collisions.

```nct
from std/io import File as StdFile
from ./my/fs import File as MyFile
```

Not adopted:

```nct
import std/io
from std/io import *
pub from std/io import *
import std/io.File
pub import std/io as io
from /absolute/path import Config
from ./config.nct import Config
use ./prelude
include std/prelude
```

Wildcard imports, bare imports without an alias, dotted import paths, namespace alias re-exports, absolute paths, explicit `.nct` extensions in import paths, project-local prelude use, and textual include are not part of the initial language.

## Re-exports

Adopted: public re-export uses `pub from`.

```nct
pub from std/string import String
pub from std/io import File as StdFile
```

`pub from path import Name` means:

- load the module at `path`
- import the public name `Name` into the current module
- expose that imported name as part of the current module's public API

Rules:

- `pub from` is allowed only at top level.
- `pub from` can re-export only public names from the source module.
- `pub(nocter)` names are not public names for `pub from`.
- Each item in a `pub from` list may independently use `as Alias`.
- Re-exported names participate in the same name collision checks as other imports and top-level declarations.
- `pub from path import *` is invalid.
- `pub import path as alias` is invalid in v0.
- `pub from` does not make private names public.
- `pub from` does not create a namespace alias.
- Import cycles involving `pub from` are still import cycles and are errors in the initial design.

## Synthetic Standard Prelude

Adopted: user project modules behave as if the compiler inserted a synthetic `use std/prelude` at the beginning of the file.

```nct
use std/prelude
```

The compiler does not rewrite source text. The synthetic prelude exists in the module/import model only. Diagnostics, formatting, AST source spans, and editor views should continue to refer to the user's original source.

The purpose is to avoid requiring this boilerplate in every file while keeping prelude behavior defined as an import rule rather than as special compiler treatment for names such as `Int`. Built-in forms such as `str`, `&str`, `[T]`, `&[T]`, and `&+[T]` are not provided by the prelude.

Initial rules:

- Every user project module has a synthetic file-local `use std/prelude`.
- The synthetic prelude is applied independently to each user project module.
- The synthetic prelude does not propagate from one file to another; each user project file gets its own synthetic prelude.
- The synthetic prelude is not applied to files inside the active Nocter home.
- The synthetic prelude is not applied to common standard-library files under `std/`.
- The synthetic prelude is not applied to `std/prelude.nct` itself.
- An explicit source-level `use std/prelude` is accepted in a user project module but is redundant.
- An explicit source-level `use std/prelude` does not introduce names twice and does not collide with the synthetic prelude.
- If a file is ineligible for the synthetic prelude, an explicit `use std/prelude` follows the normal `use` rules.
- `use std/prelude` is allowed only at top level.
- In v0, `use` is accepted only for `std/prelude`.
- `use std/prelude as prelude` is invalid.
- `use ./prelude` is invalid.
- `include std/prelude` is invalid.
- Names introduced by the synthetic or explicit prelude participate in the same collision checks as explicit imports.
- If a prelude name collides with a local declaration, top-level declaration, parameter, local binding, explicit import, or built-in name, the program is invalid.
- Diagnostics should identify collisions with the synthetic prelude as prelude collisions, not as hidden compiler built-ins.
- Project-wide prelude configuration is not part of the initial design.

Initial prelude surface direction:

```nct
pub type Int = i32

pub from std/error import Error, ErrorCode
pub from std/string import String
```

The prelude must remain small. It should contain only core type aliases and ubiquitous value-free standard-library types needed to write ordinary signatures. Names such as `File`, `Allocator`, `Layout`, `RawBuffer`, `print`, `stdout`, `stderr`, `args`, `env`, `cwd`, `exit`, and `abort` should be imported explicitly from their domain modules.

## Package Layout

Adopted: v0 has no package manifest and no project-root discovery.

The source file passed to `build`, `run`, or `check` is the root file for that command.

```text
project/
    app.nct
    src/
        config.nct
        parser.nct
```

```sh
nocter build app.nct -o app
nocter run app.nct
nocter check app.nct
```

Rules:

- A package manifest such as `nocter.toml` is not part of v0.
- The compiler does not search upward for a project root.
- The compiler does not infer a package name from a directory name.
- The root file is the `.nct` file named on the command line.
- Project-local imports must be explicit relative imports starting with `./` or `../`.
- Relative imports are resolved from the directory containing the importing file, not from the root file directory.
- Non-relative imports are resolved inside the active Nocter home as specified in [Import Path Resolution](#import-path-resolution).
- Package registries, dependency version solving, lockfiles, workspaces, and package-level configuration are not part of v0.

Example:

```nct
// app.nct
from std/io import print
from ./src/config import Config

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

pub func Config.default(): Config {
    return Config{
        name: "Nocter",
    }
}
```

## Compile Unit

`nocter build app.nct`, `nocter run app.nct`, and `nocter check app.nct` treat `app.nct` as the root file. The CLI contract is specified in [Command Line Interface](15-command-line-interface.md).

The compile unit is the root file plus every `.nct` file reached by following `from`, `pub from`, `import`, explicit `use std/prelude`, and eligible synthetic `use std/prelude` declarations recursively.

Rules:

- Import, re-export, and explicit `use` declarations are allowed only at top level.
- Synthetic `use std/prelude` is compiler-internal and behaves as if it appears before source-level imports for eligible user project modules.
- Top-level executable statements are not allowed.
- A root executable must define the active entry function in the root file.
- v0 uses `main` as the default active entry function.
- CLI `--entry <name>` overrides the active entry function for that command.
- Entry lookup does not select imported functions.
- Imported files may define ordinary functions named `main`, subject to normal name visibility and duplicate-name rules.
- The same canonical file path is loaded at most once, even if reached through different relative paths.
- Import cycles are errors in the initial design.
- The whole compile unit is name-resolved, type-checked, ownership-checked, and lowered as one program.
- Separate compilation, incremental compilation, cached module artifacts, and link-time composition of multiple Nocter compile units are not part of v0.

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

```text
Nocter home:  /Users/me/.nocter
target:       arm64-darwin
source file:  /Users/me/.nocter/std/os/macos.nct
display:      std/os/macos.nct
absolute:     /Users/me/.nocter/std/os/macos.nct
```

## Import Path Resolution

Import paths are source paths, not package names. The `.nct` extension is omitted.

Relative import paths start with `./` or `../`.

```nct
from ./config import AppConfig
from ../shared/path import Path
```

Relative paths are resolved from the directory containing the current file:

```text
current file: app/main.nct
import path:  ./config
resolved:     app/config.nct
```

Non-relative import paths start with a directory or file name.

```nct
from std/io import print
from std/fs import File
```

Non-relative paths are resolved inside the active Nocter home, normally `~/.nocter/` after user installation.

Release archive names include the host, such as `nocter-v0.1.0-arm64-darwin.tar.gz`, but the archive root is `.nocter/`. Import resolution depends on the active Nocter home path, not on the release archive filename.

For `std/...` paths, the common standard-library directory is searched:

```text
~/.nocter/std/io.nct
```

For other non-relative paths, the path is resolved directly inside Nocter home:

```text
from vendor/json import Parser

~/.nocter/vendor/json.nct
```

Rules:

- Local project imports must start with `./` or `../`.
- `from config import Config` does not search next to the current file; it searches Nocter home for `config.nct`.
- `/absolute/path` imports are errors.
- `.` is not a module separator in import paths.
- `.nct` is not written in import declarations.
- Directory modules such as `std/io/mod.nct` or `std/io/index.nct` are not part of the initial design.
- The compiler locates Nocter home from `NOCTER_HOME` if set, otherwise from the resolved real path of the running `nocter` executable and its parent directory.
- The compiler does not automatically search `cwd/.nocter` or `~/.nocter`.
- The repository development output directory `.nocter/` may act as Nocter home during local development. This is a development detail, not the user-facing installation convention.

## Name Resolution

Unqualified names are resolved inside one file after imports are loaded and visibility is checked.

Lookup order:

1. Current lexical block bindings.
2. Outer lexical block bindings.
3. Function parameters.
4. Same-file top-level declarations.
5. Explicitly imported names and names introduced by the synthetic or explicit prelude.
6. Built-in types and reserved syntax forms.

Initial rules:

- Shadowing is not allowed.
- Function parameter names must be unique within the parameter list.
- A function parameter must not reuse a visible local, parameter, top-level, imported, prelude, or built-in type name.
- A local binding must not reuse a visible local, parameter, top-level, imported, or built-in type name.
- Two imports, a re-export, or a prelude name introducing the same local name are errors.
- A same-file top-level declaration and an imported name must not have the same local name.
- `import path as alias` introduces only the alias name.
- Names inside an imported namespace alias are accessed with member syntax, such as `io.stdout()`.
- There is no wildcard import.
- There is no implicit import of every name from `std`.
- The synthetic prelude is limited to `std/prelude`; it is not a general implicit import facility.

## Visibility

Adopted: definitions are private by default. Public API is marked with `pub`. Nocter-distribution-internal API is marked with `pub(nocter)`.

```nct
pub struct File {
    fd: i32
}

pub func File.open(path: &str): File! {
    ...
}

impl File {
    method (file: &Self).raw_fd(): i32 {
        return file.fd
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
- `import` can import `pub` names from any module.
- `import` can import `pub(nocter)` names only when the importing module is inside the active Nocter home.
- User project modules cannot import `pub(nocter)` names.
- `pub from` can re-export only `pub` names from the source module as part of the current module's public API.
- `pub from` cannot re-export `pub(nocter)` names as public API.
- `pub from` re-exports the imported name as part of the current module's public API.
- Struct fields are private by default.
- Public struct fields must be marked with `pub`.
- `pub(nocter)` struct fields are visible only to modules inside the active Nocter home.
- Functions and methods are private by default.
- Public associated functions declared as `func Type.name` and public methods inside `impl` blocks must be marked with `pub`.
- Nocter-distribution-internal associated functions and methods may be marked with `pub(nocter)`.
- `impl` blocks themselves are not marked `pub`.
- Enum variants follow the visibility of their enum in the initial design.
- Trait items are deferred after v0.
- There is no `private` keyword in the initial design.
- There is no standalone `export` declaration in the initial design.
- `pub(package)`, `pub(crate)`, `pub(std)`, `pub(home)`, and `pub(trusted)` are not part of v0.

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
- Initial design does not support `mod.nct` directory modules.
- Standard library modules live under `std`.
- `~/.nocter/std/io.nct` resolves from import path `std/io` when the active Nocter home is `~/.nocter`.
- `~/.nocter/std/os/macos.nct` resolves from import path `std/os/macos`.

Import roots:

1. The current file directory for `./` and `../` paths.
2. The common standard library directory for `std/...` paths.
3. The active Nocter home root for other non-relative paths.

The compiler locates Nocter home in this order:

1. `NOCTER_HOME`, if set.
2. The directory containing the running `nocter` executable.
3. Otherwise, report a clear configuration error.
