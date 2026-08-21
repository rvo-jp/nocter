# Command Line Interface

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Package Initialization and Graph Inspection

`nocter init [DIR]` creates a source-owned package without overwriting an existing `nocter.nct`,
root `index.nct`, or generated `tests/unit/index.nct`. The directory name supplies `#name` unless
`--name` is explicit. The default template is executable; `--library` selects a library template.
Both templates declare a separate package test target and must pass `nocter check` and
`nocter test` immediately after creation.

`nocter graph` loads the same exact `PackageGraph` used by build, test, and LSP analysis. Its human
form prints package identities and labeled edges. `--format json` emits deterministic format-1
data containing package IDs, names, versions, roots, dependency source kinds, exact locks, and
resolved package IDs. `--locked` and `--offline` retain their normal resolution meaning.

Graph inspection never writes generated lock data. `nocter fetch` remains the only command that
intentionally updates dependency locks.

## Command Model

Package commands operate on a source-owned `nocter.nct`. Omitting a source selects a package; it
never guesses that `main.nct` is an executable. Explicit single-file operation remains available
for scripts and isolated experiments.

```sh
nocter --help
nocter help
nocter help check
nocter check --help
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
nocter test
nocter test --test unit
nocter test --test unit --case pushes_in_order
nocter test --locked --offline
nocter test --format json
nocter fmt app.nct
nocter fmt --check app.nct
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter lsp
```

A bare source such as `nocter app.nct` is not a command. Use `nocter run app.nct` when single-file
execution is intended.

## Help

`nocter --help` and `nocter help` produce the same overview. `nocter help <command>` and
`nocter <command> --help` produce the same command-specific report. The report lists only commands
implemented by that compiler and only options accepted by the selected command. Command names,
option spellings and aliases, value requirements, applicability, usage, and help descriptions come
from one compiler-owned command schema.

The `--help` option must be the only argument after a selected command. Help returns status 0 and
writes to stdout without selecting a Nocter home, resolving a package, or reading source.

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
- The package file declares zero or more executable and test targets.
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
- Package directives are rejected in file mode because they belong to a selected package
  `nocter.nct`.

The compiler follows imports from each selected root module to form a compile unit. Import and
source identity rules are specified in [Modules and Use Declarations](01-modules-use.md).

## Package File

The package file uses declarative directives:

```nct
//! JSON command-line tool.

#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    module: "./src/app",
}
#test: {
    name: "unit",
    module: "./tests/unit",
}
```

`#name` is presentation metadata. If absent, the package root directory basename is used as the
display name only. `#version` may be absent and is never synthesized. Each `#executable` contains a
unique package-local name and may select an explicit logical directory-module path. When `module`
is absent, the package root module at `index.nct` is selected.

Each repeatable `#test` contains a unique test name and a required logical `module`. Test and
executable names occupy different target namespaces. Test modules are never discovered from
directory names or filenames.

`nocter.nct` accepts package documentation and directives only. Ordinary imports and declarations
belong in `index.nct` or another module source.

Directive list elements and record fields are comma-delimited and may use one trailing comma on any
layout under [Comma-Delimited Lists](13-lexical-grammar.md#comma-delimited-lists).

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

Every exact dependency selection has one canonical, Windows-safe `PackageId`:

- a Git lock `git:<40-hex-commit>` becomes `git-<lowercase-40-hex-commit>`
- an archive lock `sha256:<64-hex-digest>` becomes `sha256-<lowercase-64-hex-digest>`
- a path package becomes `path-<64-lowercase-hex>`, where the digest is SHA-256 over the UTF-8
  bytes of its canonical absolute path

The Git URL and archive URL are acquisition metadata, not identity input. Two declarations that
select the same exact commit or archive content therefore select the same package even when they
use different mirrors. Symlinks in a path dependency are resolved before its identity is computed.
Display names and versions never participate in identity.

## Executable Selection

An executable declaration selects a module. The selected module must contain a top-level `func
main` with no type or value parameters and a supported process result type.

```nct
#executable: {
    name: "server",
    module: "./src/server",
}
```

Rules:

- Omitting `module` selects `.`.
- `module: "."` selects `index.nct` in the package root.
- `module: "./src/server"` selects `src/server/index.nct`.
- A module path names a directory, cannot contain `.nct`, and cannot escape the package root.
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
exact packages under `<package-root>/.nocter/packages/<PackageId>`. The directory basename is the
complete canonical `PackageId`; it is not an alias or display name. Package commands may perform
the same missing lock generation and fetch before analysis.

The complete dependency graph is validated before generated lock data is committed. A failed graph
does not partially rewrite `nocter.nct`.

- `--locked` rejects any operation that would create or change lock selection.
- `--offline` prohibits source resolution and downloads; every exact package must already exist in
  the package-local or Nocter-home store.
- Existing locks are never changed implicitly.
- LSP behaves as locked and offline regardless of command defaults.
- Resolution first checks `<package-root>/.nocter/packages/<PackageId>`, then
  `<Nocter-home>/packages/<PackageId>`. It never searches by package name, version, URL, or a
  partial identity.
- Path dependencies use their authored canonical directories directly and are not copied into an
  exact-package store.

### Acquisition Protocols

Nocter performs remote package acquisition inside the `nocter` process. It does not invoke `git`,
`curl`, or another downloader. The supported v0.14.0 remote sources are deliberately narrow:

- Git repositories fetched from a public `https://` URL
- `.tar.gz` archives fetched from a public `https://` URL

SSH Git URLs, local Git repositories, private repositories, interactive authentication, custom
certificate authorities, Git submodules, and Git LFS are unsupported. HTTPS uses normal server
certificate and hostname verification. At most five redirects may be followed, and every URL in
the redirect chain must remain HTTPS and contain no credentials.

A Git `revision` is one of a full 40-digit commit ID, `refs/heads/<name>`, `refs/tags/<name>`, or a
short branch-or-tag name. A short name must identify exactly one advertised branch or tag; a name
present in both namespaces is ambiguous. Revision-expression syntax and other reference namespaces
are unsupported. Lock generation peels the selected reference to a commit and records that exact
commit ID.

Nocter materializes the locked Git tree directly from the embedded object database, without a Git
working tree or checkout filters. It accepts regular and executable blobs plus directories. It
rejects symbolic links, submodules, and Git LFS pointer blobs. A materialized tree has the same
100,000-entry, 1-GiB regular-file-data, and 64-component path limits as an archive.

An archive lock hashes the compressed `.tar.gz` response bytes. Nocter verifies that SHA-256 digest
before decompression or extraction. The archive root itself is the package root and must directly
contain `nocter.nct`; Nocter never removes an enclosing directory automatically.

Archive extraction accepts regular files and directories only. It rejects symbolic links, hard
links, device nodes, FIFOs, absolute paths, parent-directory traversal, and duplicate destination
paths. A compressed archive may contain at most 256 MiB, 100,000 entries, 1 GiB of expanded regular
file data, and 64 path components per entry. A rejected or interrupted acquisition never publishes
a partial exact package.

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

## Test

`test` compiles and runs explicitly declared package test targets:

```sh
nocter test
nocter test --test unit
nocter test --test unit --case pushes_in_order
nocter test --locked --offline
nocter test --format json
```

`test` is package-only. It accepts `--root`, `--test`, `--case`, `--target`, `--locked`, `--offline`,
and `--format json`; it does not accept a positional source, `--file`, `--executable`, or `-o`.
`--case` requires `--test` because target and declaration names are separate typed namespaces.

Without `--test`, targets run in declaration order. Each target selects native `test name { ... }`
declarations directly contained in its selected module. Cases run in source order, or `--case` selects
one exact declaration. Every case is compiled through normal semantic, ownership, buildability,
and native-emission stages, written to a unique temporary location, and launched in its own
process. The temporary executable is removed after every outcome. A nonzero exit, signal, compile
failure, or launch failure marks that run failed and does not prevent later runs. No legacy
test-target `main` compatibility mode remains. Each process uses the selected package root as its
working directory, including when `--root` was given elsewhere.

Human output reports every run and aggregate passed/failed counts. `--format json` writes one
`nocter.tests` version-1 envelope to stdout. It contains `ok`, package and compilation-target
identity, top-level diagnostics, `runs`, and summary counts. Each run records separate `target` and
nullable source-level `test` identity, outcome (`passed`, `failed`, `compile_failed`, or
`runner_failed`), exit code or signal, captured stdout/stderr, and diagnostics. Accepted native
runs carry the exact declaration name. Target-wide failures use `test: null`; identity is never
encoded into a display string. The command exits zero only when every selected run passes.

## Check

`check` runs source-language, ownership, and selected-target buildability analysis without emitting
or executing a program.

```sh
nocter check
nocter check --executable server
nocter check --format json
nocter check app.nct --format json
```

Human-readable diagnostics go to stderr. `--format json` writes exactly one JSON diagnostic
envelope to stdout and no other stdout text. The envelope is specified in
[Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).

When they select the same executable compile unit, target, toolchain home, and dependency snapshot,
`check`, `build`, and `run` accept the same source program. `check` stops after constructing and
validating the complete target program; `build` and `run` continue through entry-driven
instantiation, lowering, and code generation. A library-only package check uses the same target
language rules but has no executable entry to instantiate. A released language feature is not
check-only, and an unfinished lowering path must not be represented as a successful public `check`
result.

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
identities, and exact source spans. Diagnostics and semantic requests share one locked, offline,
read-only package snapshot. Public editor behavior, including package manifests, native tests,
rename, code actions, hints, and incomplete-source recovery, is specified in [Tooling and Editor
Integration](14-tooling-editor-integration.md).

## Target Option

`build`, `run`, and `check` accept `--target` in either input mode. Package-only `test` also accepts
`--target`:

```sh
nocter build --target arm64-darwin
nocter run --executable server --target arm64-darwin
nocter check app.nct --target arm64-darwin
nocter test --target arm64-darwin
```

The default is the host target. `arm64-darwin` is the currently implemented target. Recognized but
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

Both commands first select and physically validate the same Nocter home used by compilation. The
installation host must equal the running compiler host. While cross compilation is unsupported,
the default target must also equal that host. Extra arguments and options are rejected.

Successful version output has this form:

```text
Nocter
release: <release>
host: <host>
default target: <target>
```

Successful doctor output has this form:

```text
Nocter home is valid
root: <canonical Nocter home path>
selected by: <NOCTER_HOME or compiler executable>
release: <release>
host: <host>
default target: <target>
```

## Output and Exit Status

Compiler-owned exit statuses are:

```text
0  success
1  source diagnostics or a formatting difference
2  command-line, filesystem, Nocter-home, or target-selection error
3  internal compiler error
```

After a program starts, `run` returns that program's exit status. `test` converts every failed test
outcome into compiler status 1 so that one target cannot terminate the runner. Human diagnostics
and command errors go to stderr. Help, `--version`, and successful `doctor` output go to stdout.

## Current Command Non-goals

- registries and semantic-version dependency resolution
- multi-package workspaces
- project-wide formatting or incremental build artifacts
- child-process argument forwarding before its separator and ownership contract are specified
