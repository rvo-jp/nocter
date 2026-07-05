# Command Line Interface

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Direction

Adopted: the `nocter` command should have a small, stable CLI contract from the beginning.

The CLI must support normal executable generation, lightweight trial execution, editor-oriented checking, and a future LSP entry point without depending on external assemblers, linkers, SDK tools, or runtime libraries.

Initial commands:

```sh
nocter build app.nct -o app
nocter run app.nct
nocter app.nct
nocter check app.nct
nocter check app.nct --format json
nocter lsp
```

## Root File

`build`, `run`, and `check` each take exactly one root `.nct` file.

Rules:

- The root file is the source file named on the command line.
- The root file must have the `.nct` extension.
- The compiler follows imports from the root file to form the compile unit.
- The compile unit rules are specified in [Modules and Imports](01-modules-imports.md#compile-unit).
- Package manifests, project-root discovery, workspaces, lockfiles, package registries, separate compilation, and incremental artifacts are not part of v0.

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
- Future program arguments, if added, should use a `--` separator to avoid ambiguity with compiler options.

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
nocter build app.nct -o app --target arm64-macos
nocter run app.nct --target arm64-macos
nocter check app.nct --target arm64-macos
```

Rules:

- If `--target` is omitted, the compiler uses the host target.
- The initial implemented target is `arm64-macos`.
- Reserved future target names may be recognized before implementation.
- Requesting a recognized but unimplemented target is a target-selection error.
- `run` can execute only targets that are runnable on the current host.
- In v0, practical `run` support is limited to `arm64-macos` on an ARM64 macOS host.

## Output Streams

Rules:

- Human-readable diagnostics go to stderr.
- Human-readable command-line and filesystem errors go to stderr.
- `build` should not print normal success output unless requested by a future verbosity option.
- `run` forwards the child program's stdout and stderr by default.
- `check --format json` writes exactly one JSON diagnostic envelope to stdout and no non-JSON text to stdout.
- `lsp` reserves stdout for LSP protocol messages.

## Exit Status

Compiler-owned exit statuses:

```text
0  success
1  source diagnostics
2  command-line, filesystem, Nocter home, target-selection, or temporary executable error
3  internal compiler error
```

Rules:

- `build` and `check` use compiler-owned exit statuses.
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
- package manifest commands
- package registry commands
- project-wide command configuration
- program argument passing before an explicit `program(args: ...)` design is adopted
