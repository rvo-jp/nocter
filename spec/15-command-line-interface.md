# Command Line Interface

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Direction

Adopted for v0.4.0 Phase 1: package commands operate on a source-owned `nocter.nct`. Omitting a
source selects a package; it never guesses that `main.nct` is an executable. Explicit single-file
operation remains available for scripts and isolated experiments.

```sh
nocter --version
nocter doctor
nocter fetch
nocter fetch --locked --offline
nocter build
nocter build --root path/to/package
nocter build --executable tool
nocter build app.nct
nocter run
nocter run --executable tool
nocter run app.nct
nocter check
nocter check --format json
nocter check app.nct
nocter fmt app.nct
nocter fmt --check app.nct
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter lsp
```

A bare source such as `nocter app.nct` is not a command. Use `nocter run app.nct` when single-file
execution is intended.

## Package and File Inputs

`build`, `run`, and `check` have two explicit input modes.

Package mode is the default:

```sh
nocter check
nocter check --root ./tools/json
```

- The default root is the current directory.
- `--root path` selects another package directory.
- The selected directory must contain `nocter.nct`.
- The package manifest declares zero or more executable targets.
- The compiler never searches for `main.nct`, walks upward for another package, or invents an
  executable target.

Single-file mode is explicit:

```sh
nocter check app.nct
nocter check --file app.nct
```

- The positional source and `--file` are equivalent.
- The file must have the `.nct` extension.
- `--root` and file mode cannot be combined.
- `--executable` cannot be used in file mode.
- Package directives are rejected in file mode because they belong to a selected package-root
  `nocter.nct`.

The compiler follows imports from each selected root module to form a compile unit. Import and
source identity rules are specified in [Modules and Use Declarations](01-modules-use.md).

## Package File

The leading package manifest uses declarative directives:

```nct
//! JSON command-line tool.

#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    entry: "./src/app",
}

pub use ./src/json
```

`#name` is presentation metadata. If absent, the package root directory basename is used as the
display name only. `#version` may be absent and is never synthesized. Each `#executable` contains a
unique package-local name and may select an explicit logical entry path without a `.nct` suffix.
When `entry` is absent, the package-root module in `nocter.nct` is the entry.

Ordinary imports and declarations after the leading directives form the package root module.
`index.nct` remains a directory module and has no package metadata responsibility.

Dependencies and their generated exact locks share `nocter.nct`:

```nct
#dependencies: {
    json: {
        git: "https://github.com/example/json.git",
        revision: "main",
    },
    http: {
        archive: "https://nocter.dev/lib/http-v1.0.0.tar.gz",
    },
    local_math: {
        path: "./packages/math",
    },
}

#lock: {
    format: 1,
    dependencies: {
        http: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        json: "git:7db21c1000000000000000000000000000000000",
    },
}
```

Git builds use only the locked commit and archives use only the locked SHA-256 content. Path
dependencies are mutable development inputs and have no lock entry. No separate lockfile exists.
The generated block is sorted by dependency alias.

## Executable Selection

An executable declaration selects a module. The selected module must contain a top-level `func
main` with no type or value parameters and a supported process result type.

```nct
#executable: {
    name: "server",
    entry: "./src/server",
}
```

Rules:

- Omitting `entry` selects `nocter.nct`.
- `entry: "."` selects `index.nct` in the package root.
- `entry: "./src/server"` selects `src/server.nct` or `src/server/index.nct`.
- If both module forms exist, selection is an error.
- A module path cannot contain `.nct` or escape the package root.
- `--executable name` selects one declared executable.
- Package `build` builds every declared executable when no name is selected.
- Package `run` selects the sole executable. Multiple declarations require `--executable`.
- Package `check` checks the root module and every executable when no name is selected. With
  `--executable`, it checks only that executable compile unit.
- A library-only package is valid for `check`; `build` and `run` report that it declares no
  executable.

## Build

`build` emits persistent native executables.

```sh
nocter build
nocter build --executable server
nocter build --executable server -o ./bin/server
nocter build app.nct -o app
```

Without `-o`, a package executable is written under the package root using its declared name. In
file mode, the output uses the source stem. `-o` requires exactly one selected executable.

`build` uses the same parser, resolver, type checker, ownership checker, buildability validation,
lowering, code generator, and executable writer as `run`. It does not invoke an external assembler,
linker, SDK tool, or runtime. Failure must not leave a partial executable at the output path.

## Fetching and Lock Control

```sh
nocter fetch
nocter fetch --root ./tools/json
nocter fetch --locked
nocter fetch --offline
```

`fetch` resolves missing direct locks, writes the generated `#lock` block atomically, and installs
exact packages under `.nocter/packages/<PackageId>`. Package commands may perform the same missing
lock generation and fetch before analysis.

The complete dependency graph is validated before generated lock data is committed. A failed graph
does not partially rewrite `nocter.nct`.

- `--locked` rejects any operation that would create or change lock selection.
- `--offline` prohibits source resolution and downloads; every exact package must already exist in
  the package-local or Nocter-home store.
- Existing locks are never changed implicitly.
- LSP behaves as locked and offline regardless of command defaults.
- Nocter home is searched only for an exact `PackageId`, never for a matching package name.

## Run

`run` builds a temporary native executable, launches it, forwards standard streams, removes the
temporary file, and returns the program's exit status.

```sh
nocter run
nocter run --executable server
nocter run app.nct
```

Compilation failures prevent launch. RAM-only execution, JIT execution, and calling `main` inside
the compiler process are not part of this contract.

## Check

`check` runs source-language and ownership analysis without emitting or executing a program.

```sh
nocter check
nocter check --executable server
nocter check --format json
nocter check app.nct --format json
```

Human-readable diagnostics go to stderr. `--format json` writes exactly one JSON diagnostic
envelope to stdout and no other stdout text. The envelope is specified in
[Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).

`check` may accept semantically valid forms whose native lowering is not yet implemented. `build`
and `run` must reject those forms during buildability validation with source-backed diagnostics.

## Format, Tokens, and AST

These commands always take exactly one source file and do not discover a package:

```sh
nocter fmt app.nct
nocter fmt --check app.nct
nocter tokens app.nct --format json
nocter ast app.nct --format json
```

- `fmt` rewrites only the named file after successful parsing. `--check` reports whether rewriting
  would be necessary.
- `tokens` exposes the compiler lexer output as a `nocter.tokens` JSON envelope.
- `ast` exposes the compiler parser output as a `nocter.ast` JSON envelope. A source-file AST
  includes package directives when present.
- These commands do not resolve dependencies, type-check, lower, emit, or execute code.

## LSP

`nocter lsp` speaks the Language Server Protocol over stdin and stdout. Protocol messages are the
only stdout data while the server is running.

The language server reuses compiler-owned parsing, resolution, types, ownership facts, declaration
identities, and exact source spans. In v0.4.0 Phase 0 it also:

- diagnoses package manifests through the same parser and validation model as package commands
- classifies directive and record-field names without coloring surrounding punctuation or space
- classifies executable entry string contents as namespaces
- resolves go-to-definition from an executable `entry` value to the selected module file
- resolves public namespace re-exports through the same declaration identity used by compilation

Rename, package-wide incremental invalidation, code actions, inlay hints, and multi-package
workspace indexing are later capabilities.

## Target Option

`build`, `run`, and `check` accept `--target` in either input mode:

```sh
nocter build --target arm64-darwin
nocter run --executable server --target arm64-darwin
nocter check app.nct --target arm64-darwin
```

The default is the host target. `arm64-darwin` is the initial implemented target. Recognized but
unimplemented targets produce target-selection diagnostics. Formatting and syntax-inspection
commands do not accept `--target`.

Declaration target gates use the source form:

```nct
#target: "arm64-darwin"
primitive syscall0(number: u64): i64
```

## Version and Doctor

`nocter --version` reports the compiler release, host, and default target. `nocter doctor` validates
the active Nocter home, including `VERSION`, `MANIFEST.json`, the host/default-target relationship,
and the standard-library directory. Neither command reads user source.

## Output and Exit Status

Compiler-owned exit statuses are:

```text
0  success
1  source diagnostics or a formatting difference
2  command-line, filesystem, Nocter-home, or target-selection error
3  internal compiler error
```

After a program starts, `run` returns that program's exit status. Human diagnostics and command
errors go to stderr. `--version` and successful `doctor` output go to stdout.

## v0.4.0 Phase 0 Non-goals

- dependency resolution, registries, package stores, lock data, or source acquisition
- multi-package workspaces
- project-wide formatting or incremental build artifacts
- test-target declarations or a package test runner
- child-process argument forwarding before its separator and ownership contract are specified

The immutable v0.2.0 command boundary remains recorded in
[Nocter v0.2.0 Completion Contract](00-v0.2.0-contract.md).
