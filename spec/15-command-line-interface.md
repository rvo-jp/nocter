# Command Line Interface

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Direction

Adopted: the `nocter` command should have a small, stable CLI contract from the beginning.

The CLI must support normal executable generation, lightweight trial execution, editor-oriented checking, source formatting, and a future LSP entry point without depending on external assemblers, linkers, SDK tools, or runtime libraries.

Initial commands:

```sh
nocter --version
nocter doctor
nocter build app.nct -o app
nocter run app.nct
nocter app.nct
nocter check app.nct
nocter check app.nct --format json
nocter fmt app.nct
nocter fmt --check app.nct
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter lsp
```

## Version

`--version` prints release identity and exits.

```sh
nocter --version
```

Rules:

- `--version` is a global option, not a subcommand.
- `--version` does not take a root file.
- `--version` does not build, run, check, or format source.
- Release compiler binaries embed the release version, host, and default target.
- The printed release version must match the `VERSION` and `MANIFEST.json` shipped in the same release archive.
- Initial human output should include at least the compiler release, host, and default target.
- `--version` does not emit JSON in v0.

Example output:

```text
Nocter 0.1.0
host: arm64-darwin
default target: arm64-darwin
```

## Doctor

`doctor` validates the active Nocter home and reports installation problems.

```sh
nocter doctor
```

Rules:

- `doctor` does not take a root file.
- `doctor` does not build, run, check, format, or execute user code.
- `doctor` resolves Nocter home using the rules in [Targets and Distribution](10-targets-distribution.md#nocter-home-resolution).
- `doctor` checks that `VERSION`, `MANIFEST.json`, `std/`, and `targets/` exist.
- `doctor` checks that `VERSION` matches `MANIFEST.json`'s `release`.
- `doctor` checks that `MANIFEST.json.host` matches the running compiler's host.
- `doctor` checks that `MANIFEST.json.default_target` is listed in `MANIFEST.json.implemented_targets`.
- `doctor` checks that each implemented target listed in `MANIFEST.json.implemented_targets` has an existing `std_path`.
- `doctor` checks that the common `std/` directory is present.
- v1 does not check a compiler binary checksum because `MANIFEST.json` v1 does not include checksum metadata.
- Human-readable output goes to stdout on success and stderr on failure.
- `doctor` exits with status `0` when the installation is valid.
- `doctor` exits with status `2` for Nocter home, filesystem, manifest, or installation errors.

## Root File

`build`, `run`, and `check` each take exactly one root `.nct` file.

Rules:

- The root file is the source file named on the command line.
- The root file must have the `.nct` extension.
- The compiler follows imports from the root file to form the compile unit.
- The compile unit rules are specified in [Modules and Imports](01-modules-imports.md#compile-unit).
- Package manifests, project-root discovery, workspaces, lockfiles, package registries, separate compilation, and incremental artifacts are not part of v0.

`fmt` takes one `.nct` source file but does not treat it as a compile-unit root. It formats only the file named on the command line.

## Build

`build` generates a persistent executable.

```sh
nocter build app.nct -o app
```

Rules:

- `build` runs lexing, parsing, name resolution, type checking, ownership checking, target lowering, ARM64 code generation, and Mach-O executable generation.
- `-o path` sets the executable output path.
- If `-o` is omitted, the initial driver may derive an output path from the root file stem.
- `build` must not invoke `clang`, `as`, `ld`, Xcode Command Line Tools, or an external linker.
- On success, `build` leaves an executable at the output path.
- On failure, `build` must not leave a partial or corrupt executable at the output path.
- The implementation should write to a temporary file in the destination directory and atomically replace the output path only after executable generation succeeds.

## Run

`run` is the lightweight trial execution command.

```sh
nocter run app.nct
```

Adopted: `run` uses a temporary Mach-O executable, not RAM-only execution.

Meaning:

```text
source.nct
    -> nocter
    -> temporary Mach-O executable
    -> spawn / exec
    -> remove temporary executable
```

Rules:

- `run` uses the same front end, semantic checks, target lowering, ARM64 code generation, and Mach-O writer as `build`.
- `run` does not create a persistent project output file.
- `run` creates a temporary executable in a private temporary location.
- The temporary executable is a real Mach-O executable for the active target.
- The temporary executable is removed after the executed program exits.
- If compilation fails, no program is executed.
- If temporary executable creation fails, the command reports a command-line or filesystem error.
- RAM-only execution, JIT execution, and calling `program` inside the compiler process are not part of v0.
- `run` must not require external tools.
- `run` forwards the executed program's standard input, standard output, and standard error by default.

Rationale:

- macOS process execution is path-based for normal executable launch.
- A temporary executable keeps `run` behavior close to `build`.
- The compiler validates the same Mach-O path used for persistent executables.
- Process exit, signals, standard streams, working directory, and environment are handled by normal OS process execution instead of an in-process JIT model.

## Shorthand Run

The command:

```sh
nocter app.nct
```

is equivalent to:

```sh
nocter run app.nct
```

Rules:

- The shorthand is recognized only when no explicit subcommand is present and the first positional argument is a `.nct` root file.
- The shorthand exists for quick local trials.
- Documentation should teach `nocter run app.nct` as the explicit form.
- Future program arguments for `run`, if added, use a `--` separator to avoid ambiguity with compiler options.
- The reserved future shape is `nocter run app.nct -- arg1 arg2`.
- The shorthand form may mirror that as `nocter app.nct -- arg1 arg2`.

## Check

`check` validates a program without emitting or executing an executable.

```sh
nocter check app.nct
nocter check app.nct --format json
```

Rules:

- `check` runs lexing, parsing, name resolution, type checking, ownership checking, target selection, and target validation needed to produce the same source diagnostics as `build`.
- `check` does not emit an executable.
- `check` does not execute user code.
- `--format human` is the default format.
- `--format json` emits machine-readable diagnostics suitable for editor integrations.
- With `--format json`, stdout must contain exactly one JSON diagnostic envelope and no other text.
- With `--format json`, human-readable progress messages must not be printed to stdout.
- Human-readable diagnostics are written to stderr.

The JSON diagnostic envelope is specified in [Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).

The root path and source span path rules are specified in [Modules and Imports](01-modules-imports.md#source-file-identity).

Initial implementation note:

- The first `check --format json` implementation may stop after source loading, lexing, parsing, typed AST construction, `program` entry validation, and basic return checking.
- Later implementations should extend the same command with import resolution, name resolution, type checking, ownership checking, target selection, and target validation.

## Format

`fmt` formats a source file using the official Nocter source style.

```sh
nocter fmt app.nct
nocter fmt --check app.nct
```

Rules:

- `fmt` takes exactly one `.nct` source file in v0.
- `fmt` formats only the named source file.
- `fmt` does not follow imports.
- `fmt` does not perform name resolution, type checking, ownership checking, target lowering, code generation, or execution.
- `fmt` parses enough syntax to preserve comments and produce valid Nocter source.
- If parsing fails, `fmt` reports diagnostics and must not rewrite the file.
- `fmt` rewrites the source file in place only after formatting succeeds.
- `fmt --check` reports whether the file already matches formatter output and does not rewrite the file.
- `fmt` has no target option.

The formatting rules are specified in [Source Style and Formatting](16-source-style-formatting.md).

## Tokens

`tokens` prints the compiler lexer output for one source file.

```sh
nocter tokens app.nct --format json
```

Rules:

- `tokens` takes exactly one `.nct` source file in v0.
- `tokens` is JSON-only in v0; `--format json` is required.
- `tokens` does not follow imports.
- `tokens` does not parse, resolve names, type-check, ownership-check, target-lower, codegen, emit an executable, or execute user code.
- `tokens` uses the same lexer as `build`, `run`, and `check`.
- The JSON output is intended for compiler debugging, editor integration, tests, and AI-assisted code repair.
- The JSON output must include enough source span information to map tokens back to the input file.

The token vocabulary follows [Lexical Grammar](13-lexical-grammar.md).

Initial token JSON envelope:

```json
{
  "schema": "nocter.tokens",
  "version": 1,
  "ok": true,
  "command": "tokens",
  "file": "app.nct",
  "absolute_path": "/Users/me/project/app.nct",
  "tokens": [
    {
      "kind": "keyword",
      "lexeme": "program",
      "span": {
        "file": "app.nct",
        "absolute_path": "/Users/me/project/app.nct",
        "start_byte": 0,
        "end_byte": 7,
        "start_line": 1,
        "start_column_byte": 1,
        "end_line": 1,
        "end_column_byte": 8
      }
    }
  ],
  "diagnostics": []
}
```

Token JSON rules:

- `schema` is `"nocter.tokens"`.
- `version` is `1`.
- `ok` is `true` only when lexing succeeded without diagnostics.
- `file` is the input display path.
- `absolute_path` is the canonical absolute path when known, or `null`.
- `tokens` is an array of token objects.
- `kind` is the compiler token kind name.
- `lexeme` is the exact normalized source text covered by the token.
- `span` uses the public JSON span shape from [Diagnostics](12-diagnostics.md#source-and-span-model).
- `diagnostics` uses the diagnostic object shape from [Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).

## AST

`ast` prints the compiler parser output for one source file.

```sh
nocter ast app.nct --format json
```

Rules:

- `ast` takes exactly one `.nct` source file in v0.
- `ast` is JSON-only in v0; `--format json` is required.
- `ast` parses one source file and does not follow imports.
- `ast` does not resolve names, type-check, ownership-check, target-lower, codegen, emit an executable, or execute user code.
- `ast` uses the same lexer and parser as `build`, `run`, and `check`.
- The JSON output is intended for compiler debugging, editor integration, tests, and AI-assisted code repair.
- The JSON output must include source spans for syntax nodes when practical.
- AST JSON is a compiler tooling format, not a stable serialization of the language specification.

The AST command direction is part of [Tooling and Editor Integration](14-tooling-editor-integration.md#ai-assisted-tooling-stage).

Initial AST JSON envelope:

```json
{
  "schema": "nocter.ast",
  "version": 1,
  "ok": true,
  "command": "ast",
  "file": "app.nct",
  "absolute_path": "/Users/me/project/app.nct",
  "ast": {
    "kind": "source_file",
    "span": {
      "file": "app.nct",
      "absolute_path": "/Users/me/project/app.nct",
      "start_byte": 0,
      "end_byte": 34,
      "start_line": 1,
      "start_column_byte": 1,
      "end_line": 3,
      "end_column_byte": 2
    },
    "items": []
  },
  "diagnostics": []
}
```

AST JSON rules:

- `schema` is `"nocter.ast"`.
- `version` is `1`.
- `ok` is `true` only when parsing succeeded without diagnostics.
- `file` is the input display path.
- `absolute_path` is the canonical absolute path when known, or `null`.
- `ast` is an object when parsing succeeds and `null` when parsing fails before a useful tree exists.
- AST node objects include `kind` and should include `span` when practical.
- AST node objects may include `value` for compact leaf information such as identifiers, literals, import paths, and type names.
- AST node spans use the public JSON span shape from [Diagnostics](12-diagnostics.md#source-and-span-model).
- `diagnostics` uses the diagnostic object shape from [Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).
- AST JSON is allowed to change while the AST is internal and unstable; AI tools should treat it as a tooling aid, not a source compatibility promise.

## LSP

`lsp` starts the future language server.

```sh
nocter lsp
```

Rules:

- `lsp` does not build, run, or check a single root file as a one-shot command.
- `lsp` speaks the Language Server Protocol over standard input and standard output.
- LSP protocol messages are the only data written to stdout while the server is running.
- Human-readable server logs, if any, must go to stderr or a configured log file.
- `lsp` reuses the compiler lexer, parser, resolver, type checker, ownership checker, and diagnostics.

The editor integration direction is specified in [Tooling and Editor Integration](14-tooling-editor-integration.md).

## Target Option

`build`, `run`, and `check` accept an optional target:

```sh
nocter build app.nct -o app --target arm64-darwin
nocter run app.nct --target arm64-darwin
nocter check app.nct --target arm64-darwin
```

Rules:

- If `--target` is omitted, the compiler uses the host target.
- The initial implemented target is `arm64-darwin`.
- Reserved future target names may be recognized before implementation.
- Requesting a recognized but unimplemented target is a target-selection error.
- `run` can execute only targets that are runnable on the current host.
- In v0, practical `run` support is limited to `arm64-darwin` on an ARM64 macOS host.
- `fmt` does not accept `--target` because source style is target-independent.
- `tokens` and `ast` do not accept `--target` because lexing and parsing are target-independent.

## Output Streams

Rules:

- Human-readable diagnostics go to stderr.
- Human-readable command-line and filesystem errors go to stderr.
- `--version` writes version information to stdout.
- `doctor` writes a success summary to stdout and failure details to stderr.
- `build` should not print normal success output unless requested by a future verbosity option.
- `run` forwards the child program's stdout and stderr by default.
- `check --format json` writes exactly one JSON diagnostic envelope to stdout and no non-JSON text to stdout.
- `fmt --check` may print human-readable formatting differences or a summary to stderr.
- `tokens --format json` writes exactly one JSON token envelope to stdout and no non-JSON text to stdout.
- `ast --format json` writes exactly one JSON AST envelope to stdout and no non-JSON text to stdout.
- `lsp` reserves stdout for LSP protocol messages.

## Exit Status

Compiler-owned exit statuses:

```text
0  success
1  source diagnostics or `fmt --check` formatting difference
2  command-line, filesystem, Nocter home, target-selection, or temporary executable error
3  internal compiler error
```

Rules:

- `build` and `check` use compiler-owned exit statuses.
- `--version` uses compiler-owned exit statuses.
- `doctor` uses compiler-owned exit statuses.
- `fmt` uses compiler-owned exit statuses.
- `fmt --check` exits with status `1` when the file would be changed by formatter output.
- `lsp` uses compiler-owned exit statuses when startup fails.
- After `run` successfully starts the compiled program, the `nocter run` process exits with the executed program's exit status.
- If `run` fails before starting the program, it uses compiler-owned exit statuses.
- If the executed program terminates by signal, `run` follows the host platform's conventional process-status reporting.

## Not Adopted in v0

The following are not part of v0:

- RAM-only execution
- JIT execution
- in-process execution of Nocter `program`
- external assembler or linker fallback
- project-wide formatting
- package manifest commands
- package registry commands
- project-wide command configuration
- passing `run` child-process arguments before the `--` forwarding design is implemented
- `program(args: ...)` entry signatures
