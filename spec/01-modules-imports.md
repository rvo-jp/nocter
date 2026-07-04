# Modules and Imports

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Modules

Adopted: Nocter modules are derived from file paths. The language does not have a `module` declaration.

A module name is derived from the source file path relative to an import root:

```text
examples/word_count.nct => examples.word_count
std/io.nct              => std.io
```

The module name is a namespace. It groups related definitions and prevents accidental name collisions. The file path is the source of truth; there is no separate module name inside the file.

Imports make names from another module available.

```nct
import std.io.{File, stdout}
import std.mem.Allocator
```

Adopted import forms:

```nct
import std.mem.Allocator
import std.io.{File, stdout, stderr}
import std.io as io
import std.io.File as StdFile
```

Meaning:

- `import module.Name` imports a single exported name into the local import scope.
- `import module.{NameA, NameB}` imports multiple exported names from one module.
- `import module as alias` imports the module under an alias.
- `import module.Name as Alias` imports one exported name under an alias.

Examples:

```nct
import std.io as io
import std.io.File as StdFile

var out = io.stdout()
let file = try StdFile.open(path)
```

Name collisions are compile errors.

```nct
import std.io.File
import my.fs.File
// error: File is imported twice
```

Use aliases to resolve collisions.

```nct
import std.io.File as StdFile
import my.fs.File as MyFile
```

Not adopted:

```nct
import std.io.*
import ./foo
import ../bar
```

Wildcard imports, relative imports, and implicit-all imports are not part of the initial language.

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
- `import` can import only public names from another module.
- Struct fields are private by default.
- Public struct fields must be marked with `pub`.
- Functions and methods inside `impl` blocks are private by default.
- Public associated functions and methods must be marked with `pub`.
- `impl` blocks themselves are not marked `pub`.
- Enum variants follow the visibility of their enum in the initial design.
- Trait items follow the visibility of their trait in the initial design.
- There is no `private` keyword in the initial design.
- There is no `export` declaration in the initial design.

Example:

```nct
pub struct Point {
    pub x: Int
    pub y: Int
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
- `/` in the relative path becomes `.` in the module name.
- The `.nct` extension is removed.
- File and directory names used for modules must be snake_case identifiers.
- `module` is not a keyword.
- Initial design does not support `mod.nct` directory modules.
- Standard library modules live under `std`.
- `.nocter-arm64-macos/std/io.nct` resolves as `std.io` when the active Nocter home is `.nocter-arm64-macos`.
- `.nocter-arm64-macos/targets/arm64-macos/std/os/macos.nct` resolves as `std.os.macos` when the active target is `arm64-macos`.

Import roots:

1. The current project root.
2. The active target standard-library overlay, normally `~/.nocter-arm64-macos/targets/arm64-macos/std` for the initial target.
3. The common standard library directory, normally `~/.nocter-arm64-macos/std` for the initial host package.

If a `std.*` module exists in both the active target overlay and the common standard library, the active target overlay wins. This allows a target to replace an entire standard-library module when a shared implementation is not suitable.

The compiler locates Nocter home in this order:

1. `NOCTER_HOME`, if set.
2. The directory containing the running `nocter` executable.
3. Otherwise, report a clear configuration error.
