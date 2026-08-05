# Source-Native Package Roots

## Responsibility Boundary

`index.nct` owns both the root module's public API and the package header. The package loader reads
the header before resolving imports; the ordinary frontend receives already selected root modules.
Package discovery, target selection, source loading, semantic analysis, and artifact emission remain
separate responsibilities.

The root file has three ordered regions:

1. file documentation using `//!` or `/*! ... */`
2. package directives
3. ordinary imports, re-exports, and declarations

Package directives are valid only in the `index.nct` selected as a package root. A nested
`index.nct` remains an ordinary directory module.

## Declarative Directive Values

Directives use `#name: value`. Values are declarative data, not Nocter expressions. The value model
contains strings, integers, booleans, lists, and records. It has no name lookup, calls,
interpolation, allocation, target execution, or user-defined extension point.

```nct
#name: "json-tool"
#version: "0.1.0"

#executable: {
    name: "json-tool",
    module: "./src/app",
}
```

`#name` and `#version` occur at most once. `#executable` is repeatable. Unknown fields, duplicate
fields, duplicate executable names, wrong value kinds, and unsupported directives are errors with
field-precise spans.

Declaration directives use the same spelling but remain a separate AST responsibility:

```nct
#target: "arm64-darwin"
primitive syscall0(number: u64): i64
```

## Package Identity

A display name is not a semantic identity. The compiler models at least:

```text
PackageId
ModuleId      = PackageId + canonical logical module path
ExecutableId  = PackageId + executable name
```

For a local root, the selected canonical root establishes package identity for one analysis
snapshot. `#name` is used in diagnostics, output naming, and future publication metadata. When it
is absent, the root directory basename supplies only that presentation value.

Module paths are logical paths. An executable declares `"./src/app"`, never
`"./src/app.nct"`. Normal resolution selects `src/app.nct` or `src/app/index.nct` and diagnoses an
ambiguity if both exist. Canonicalization must keep both the selected `index.nct` and executable
modules within the canonical package root; symbolic links cannot bypass the root boundary.

## Executable Targets

Each executable declaration has a package-local name and module:

```nct
#executable: {
    name: "json-tool",
    module: "./src/app",
}
```

The selected module must contain the ordinary Nocter entry declaration `func main`. Phase 0 does
not configure another function name. `module: "."` selects the package root itself.

Library-only packages omit `#executable`. They remain valid inputs to `nocter check`; commands that
require an executable report the missing target instead of probing for `main.nct`.

## Command Selection

Package commands select `index.nct` from the explicit working root or root option. `build` builds
all declared executables unless one is selected. `run` selects the sole executable or requires its
name when more than one exists.

An explicit single-file source after a command may remain for scripts, but it creates an ephemeral
package and uses the same frontend and executable model. Bare `.nct` commands do not silently
switch the package command contract, and omitting a source always selects `index.nct` rather than
probing for `main.nct`.

## Public Namespace Re-exports

```nct
pub use ./src/json
```

re-exports the resolved module namespace under its default final-segment name. It does not import
or flatten every public declaration. Explicit name re-exports remain separate:

```nct
pub use ./src/json.{Json, parse}
```

Namespace re-exports participate in visibility, collision, definition, references, hover,
completion, semantic tokens, and public API documentation through one resolved declaration
identity.

## Dependency Boundary

The future dependency form declares one dependency edge per directive:

```nct
#depend: {
    name: "json",
    git: "https://github.com/foo/json.git",
    commit: "7db21c1...",
}
```

`name` will be the importing package's local namespace, not the dependency's semantic identity.
Phase 0 does not implement this edge. A later phase must add declaration, resolution, acquisition,
cache, cycle, offline, and LSP behavior as one coherent capability.
