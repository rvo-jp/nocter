# Packages and Package Source

This chapter defines package identity, package-root source, dependency declarations, and executable
and test target declarations. [Modules, Use Declarations, and Source Visibility](modules.md) defines
the directory modules selected by those declarations. The
[Command-Line Interface](../tooling/command-line.md) defines how commands consume and update this
source contract.

## Package Root

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
- package directives are invalid outside the package root `index.nct`
- a descendant `index.nct` containing `#package` starts a nested package; one without `#package`
  starts a child module

The compiler does not discover a package target by probing `main.nct` or another conventional
filename.

## Directive Data

Directive records and lists use the data notation recognized by
[Syntactic Grammar](syntactic-grammar.md#package-directives). List elements and record fields are
comma-delimited and may use one trailing comma on any layout under
[Comma-Delimited Lists](lexical-grammar.md#comma-delimited-lists).

The package directive names form this closed set:

```text
package
dependencies
executable
test
```

Unknown directive names are invalid. In particular, `#lock` is not a package directive and no
separate lockfile exists.

## Dependencies

Each dependency record owns both authored source intent and its optional generated exact selection:

```nct
#dependencies: {
    json: {
        git: "https://github.com/example/json.git",
        revision: "main",
        commit: "7db21c1000000000000000000000000000000000",
    },
    http: {
        archive: "https://nocter.dev/lib/http-v1.0.0.tar.gz",
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    },
    local_math: {
        path: "./packages/math",
    },
}
```

Dependency aliases are package-local names. Reserved alias `std` cannot be declared because the
active toolchain supplies it implicitly.

Exactly one source form is selected by each dependency entry:

- `git` requires authored `revision`; `commit` is its optional exact selection
- `archive` accepts `sha256` as its optional exact selection
- `path` selects a mutable local package and accepts no exact-selection field

`git`, `archive`, and `path` are mutually exclusive. Fields belonging to a different source form,
unknown fields, and wrong value kinds are invalid.

Git builds use only an exact lowercase 40-hex `commit`. Archive builds use only content with the
exact lowercase 64-hex `sha256`. Path dependencies remain mutable development inputs. Dependency
acquisition and exact-field updates are command behavior defined by
[`nocter fetch`](../tooling/command-line.md#fetching-and-lock-control).

Every exact dependency selection has one canonical, Windows-safe package identity:

- a Git `commit` becomes `git-<lowercase-40-hex-commit>`
- an archive `sha256` becomes `sha256-<lowercase-64-hex-digest>`
- a path package becomes `path-<64-lowercase-hex>`, where the digest is SHA-256 over the UTF-8
  bytes of its canonical absolute path

The Git URL and archive URL are acquisition metadata, not identity input. Two declarations that
select the same exact commit or archive content select the same package even when they use different
mirrors. Symlinks in a path dependency are resolved before its identity is computed. Display names
and versions never participate in identity.

## Executable Targets

`#executable` is repeatable. Each record requires a package-local `name` unique among executable
targets and accepts an optional `module`:

```nct
#executable: {
    name: "server",
    module: "./src/server",
}
```

An omitted `module` selects `.`. Target module paths are `.` or package-relative directory paths
beginning with `./`:

- `.` selects the package root `index.nct`
- `./src/server` selects `src/server/index.nct`
- paths omit `.nct`, cannot escape the package, and cannot cross a nested package
- targets never select ordinary implementation sources

Executable and test names occupy separate target namespaces. Command selection and executable-entry
validation are defined by [Executable Selection](../tooling/command-line.md#executable-selection).

## Test Targets

`#test` is repeatable. Each record requires a package-local `name` unique among test targets and a
required `module`:

```nct
#test: {
    name: "unit",
    module: "./tests/unit",
}
```

Test target module paths follow the same directory-module rules as executable targets. Test modules
are never discovered from directory names or filenames. Test execution behavior is defined by
[Native Testing](../tooling/testing.md).

## Implicit Standard-Library Package

The active Nocter home contributes one immutable package at `<Nocter-home>/std`. Its root
`index.nct` contains `#package`; the package name and version must match the toolchain installation.
Every compilation graph binds reserved dependency alias `std` to this exact package, including
imports written inside `std` itself and single-file mode.

A package named `std`, a directory with that spelling, or a dependency alias cannot shadow the
compiler-selected package or gain its primitive authority. Single-file mode uses the toolchain
package without creating a package declaration for the source file.

The standard package follows ordinary directory-module, visibility, `use`, and `see` rules. Its
special authority is limited to the trusted declarations and primitive boundaries specified in
[Standard Library Primitives and OS Boundaries](../platform/primitives-and-os.md).
