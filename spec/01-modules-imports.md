# Modules and Imports

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Modules

Adopted: Nocter modules are derived from file paths. The language does not have a `module` declaration.

A module identity is derived from the canonical source file path. One `.nct` file defines one module.

Import paths use `/` as the path separator:

```text
examples/word_count.nct                                  => examples/word_count
~/.nocter/std/io.nct                                     => std/io
~/.nocter/targets/arm64-macos/std/os/macos.nct           => std/os/macos
```

The file path is the source of truth. There is no separate module name inside the file.

Imports make names from another module available.

```nct
from std/io import File, stdout
from std/mem import Allocator
```

Adopted import forms:

```nct
use std/prelude

from std/mem import Allocator
from std/io import File, stdout, stderr
from std/io import File as StdFile
from ./config import AppConfig
from ../shared/path import Path
pub from std/string import String, StringView

import std/io as io
```

Meaning:

- `use std/prelude` explicitly enables the standard prelude for the current file.
- `from path import Name` imports one exported name into the current file.
- `from path import NameA, NameB` imports multiple exported names from one file.
- `from path import Name as Alias` imports one exported name under an alias.
- Each imported item in a `from` list may independently use `as Alias`.
- `pub from path import Name` imports and re-exports one public name.
- `import path as alias` imports the module namespace under an alias.

Examples:

```nct
use std/prelude

import std/io as io
from std/io import File as StdFile

var out = io.stdout()
let file = try StdFile.open(path)
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
pub from std/string import String, StringView
pub from std/io import File as StdFile
```

`pub from path import Name` means:

- load the module at `path`
- import the public name `Name` into the current module
- expose that imported name as part of the current module's public API

Rules:

- `pub from` is allowed only at top level.
- `pub from` can re-export only public names from the source module.
- Each item in a `pub from` list may independently use `as Alias`.
- Re-exported names participate in the same name collision checks as other imports and top-level declarations.
- `pub from path import *` is invalid.
- `pub import path as alias` is invalid in v0.
- `pub from` does not make private names public.
- `pub from` does not create a namespace alias.
- Import cycles involving `pub from` are still import cycles and are errors in the initial design.

## Explicit Prelude

Adopted: Nocter uses an explicit file-local prelude statement.

```nct
use std/prelude
```

`use std/prelude` imports the public prelude names from `std/prelude.nct` into the current file's import scope.

Initial rules:

- `use std/prelude` is optional.
- `use std/prelude` is allowed only at top level.
- `use std/prelude` affects only the file that contains it.
- `use std/prelude` does not propagate to imported files.
- A file may contain at most one `use std/prelude` statement.
- In v0, `use` is accepted only for `std/prelude`.
- `use std/prelude as prelude` is invalid.
- `use ./prelude` is invalid.
- `include std/prelude` is invalid.
- Names introduced by `use std/prelude` participate in the same collision checks as explicit imports.
- If a prelude name collides with a local declaration, top-level declaration, parameter, local binding, explicit import, or built-in name, the program is invalid.
- There is no implicit prelude.
- Project-wide prelude configuration is not part of the initial design.

Initial prelude surface direction:

```nct
pub type Int = i32

pub from std/string import String, StringView
pub from std/view import View, WriteView
```

The prelude should remain small. Names such as `File`, `IOError`, `print`, `args`, `exit`, and `abort` should be imported explicitly from their domain modules.

## Compile Unit

`nocter build app.nct` treats `app.nct` as the root file.

The compile unit is the root file plus every `.nct` file reached by following imports recursively.

Rules:

- Import, re-export, and `use` declarations are allowed only at top level.
- Top-level executable statements are not allowed.
- A root executable must contain exactly one `program` construct.
- `program` is allowed only in the root file.
- Imported files must not define `program`.
- The same canonical file path is loaded at most once.
- Import cycles are errors in the initial design.

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

The archive payload may be named `.nocter-<host>/`, such as `.nocter-arm64-macos/`, but once installed and renamed to `~/.nocter/`, import resolution must not depend on the payload directory name.

For `std/...` paths, the active target overlay is searched before the common standard library:

```text
~/.nocter/targets/arm64-macos/std/io.nct
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
- If a `std/...` file exists in both the active target overlay and common `std/`, the active target overlay wins.
- The compiler locates Nocter home from `NOCTER_HOME` if set, otherwise from the directory containing the running `nocter` executable.
- The repository development output directory `.nocter-arm64-macos/` may act as Nocter home during local development. This is a development detail, not the user-facing installation convention.

## Name Resolution

Unqualified names are resolved inside one file after imports are loaded and visibility is checked.

Lookup order:

1. Current lexical block bindings.
2. Outer lexical block bindings.
3. Function parameters.
4. Same-file top-level declarations.
5. Explicitly imported names and names introduced by `use std/prelude`.
6. Built-in types and reserved syntax forms.

Initial rules:

- Shadowing is not allowed.
- Function parameter names must be unique within the parameter list.
- A function parameter must not reuse a visible local, parameter, top-level, imported, prelude, or built-in type name.
- A local binding must not reuse a visible local, parameter, top-level, imported, or built-in type name.
- Two imports, a re-export, or `use std/prelude` introducing the same local name are errors.
- A same-file top-level declaration and an imported name must not have the same local name.
- `import path as alias` introduces only the alias name.
- Names inside an imported namespace alias are accessed with member syntax, such as `io.stdout()`.
- There is no wildcard import.
- There is no implicit import of every name from `std`.

## Visibility

Adopted: definitions are private by default. Public API is marked with `pub`.

```nct
pub struct File {
    fd: i32
}

impl File {
    pub func open(path: StringView): File!IOError {
        ...
    }

    method (file: &Self).raw_fd(): i32 {
        return file.fd
    }
}

pub func stdout(): File {
    ...
}
```

Rules:

- Top-level definitions are private to their module by default.
- `pub` on a top-level definition makes it importable from other modules.
- Type aliases are top-level definitions. `type Name = Target` is private by default, and `pub type Name = Target` makes the alias importable and re-exportable.
- `import` and `pub from` can import only public names from another module.
- `pub from` re-exports the imported name as part of the current module's public API.
- Struct fields are private by default.
- Public struct fields must be marked with `pub`.
- Functions and methods inside `impl` blocks are private by default.
- Public associated functions and methods must be marked with `pub`.
- `impl` blocks themselves are not marked `pub`.
- Enum variants follow the visibility of their enum in the initial design.
- Trait items follow the visibility of their trait in the initial design.
- There is no `private` keyword in the initial design.
- There is no standalone `export` declaration in the initial design.

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
- File and directory names used for modules must be snake_case identifiers.
- `module` is not a keyword.
- Initial design does not support `mod.nct` directory modules.
- Standard library modules live under `std`.
- `~/.nocter/std/io.nct` resolves from import path `std/io` when the active Nocter home is `~/.nocter`.
- `~/.nocter/targets/arm64-macos/std/os/macos.nct` resolves from import path `std/os/macos` when the active target is `arm64-macos`.

Import roots:

1. The current file directory for `./` and `../` paths.
2. The active target standard-library overlay for `std/...` paths.
3. The common standard library directory for `std/...` paths.
4. The active Nocter home root for other non-relative paths.

If a `std/...` module exists in both the active target overlay and the common standard library, the active target overlay wins. This allows a target to replace an entire standard-library module when a shared implementation is not suitable.

The compiler locates Nocter home in this order:

1. `NOCTER_HOME`, if set.
2. The directory containing the running `nocter` executable.
3. Otherwise, report a clear configuration error.
