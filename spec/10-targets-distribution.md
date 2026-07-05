# Targets and Distribution

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Target Model

Adopted: Nocter has one initial target, but the compiler architecture should keep target-specific behavior isolated.

Initial target:

```text
arm64-macos
```

Initial target properties:

- CPU architecture: ARM64
- OS: macOS
- executable format: Mach-O
- pointer width: 64-bit
- `usize`: `u64` range
- `isize`: `i64` range

Rules:

- The initial compiler implementation targets only `arm64-macos`.
- The initial implementation does not support cross compilation beyond `arm64-macos`, but the compiler still models host and target separately.
- The default active target is the host target.
- The language grammar, type system, ownership model, borrow rules, regions, and high-level standard-library APIs should not depend on macOS-specific names.
- Target-specific logic belongs in target backends, primitive lowering, executable writers, and OS-specific standard-library modules.
- The compiler must not depend on external assemblers, linkers, C toolchains, or external runtimes for any target.
- Future targets should be added by introducing new target backends and target-specific standard-library primitive boundaries.
- Future targets must not require ordinary user code to mention CPU instructions, object formats, or OS syscall details.
- Future cross compilation is selected by an explicit target option such as `--target x64-linux`.
- A recognized target name is not the same as an implemented target. A target becomes implemented only when its backend, executable writer, primitive set, and target standard-library overlay exist.

Current target-specific standard-library boundary:

```text
~/.nocter/targets/arm64-macos/std/os/macos.nct
```

Future target-specific boundaries may use parallel target overlays such as:

```text
~/.nocter/targets/x64-linux/std/os/linux.nct
~/.nocter/targets/arm64-linux/std/os/linux.nct
~/.nocter/targets/x64-windows/std/os/windows.nct
~/.nocter/targets/arm64-windows/std/os/windows.nct
```

These future files are not part of the initial implementation goal.

Recognized targets:

```text
arm64-macos    implemented first
x64-linux      reserved, not implemented
arm64-linux    reserved, not implemented
x64-windows    reserved, not implemented
arm64-windows  reserved, not implemented
```

If a reserved target is requested before implementation, the compiler must report a clear error:

```text
error: target x64-linux is recognized but not implemented
```

## Distribution Layout

Adopted: the downloadable archive is host-specific, but the normal user installation directory is host-independent.

The initial archive name and payload are:

```text
nocter-<version>-arm64-macos.tar.gz

.nocter-arm64-macos/
    nocter
    std/
    targets/
```

Users install the payload by moving or renaming it to `~/.nocter/`.

The installed layout is:

```text
~/.nocter/
    nocter
    std/
        prelude.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
        view.nct
    targets/
        arm64-macos/
            std/
                process.nct
                os/
                    macos.nct
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

The `host` part in the archive and payload name identifies the environment that runs the `nocter` compiler binary. The first host is `arm64-macos`. Future downloaded payloads may include names such as `.nocter-x64-linux/` or `.nocter-arm64-linux/`, but the recommended installed directory remains `~/.nocter/`.

The installed Nocter home contains common standard-library source files and one or more target overlays under `targets/<target>/`.

Because cross compilation beyond `arm64-macos` is not part of the initial implementation, the default active target is the host target. For example, the `arm64-macos` archive contains the compiler that runs on ARM64 macOS, and `targets/arm64-macos/` contains the standard-library primitive boundary for the `arm64-macos` target.

Future cross compilation adds target overlays and compiler backends to the installed Nocter home:

```text
~/.nocter/
    nocter
    std/
    targets/
        arm64-macos/
            std/
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

Initial command-line direction:

```sh
nocter build app.nct
nocter build app.nct -o app
nocter run app.nct
nocter app.nct
nocter check app.nct
nocter check app.nct --format json
nocter lsp
nocter build app.nct --target arm64-macos
nocter build app.nct --target x64-linux
```

The command-line contract is specified in [Command Line Interface](15-command-line-interface.md).

`build`, `run`, and `check` each take one root `.nct` file. The compiler follows imports from that root file to form the compile unit; it does not read a package manifest in v0.

`-o path` sets the executable output path. If `-o` is omitted, the initial driver may derive an output path from the root file stem.

If `--target` is omitted, the compiler uses the host target. The initial implementation can emit only `arm64-macos`. Reserved targets may be recognized by name, but they must produce a not-implemented diagnostic until their backend, executable writer, primitive set, and target standard-library overlay are implemented.

Build profile direction:

- The initial language semantics do not define different safety levels for debug and release builds.
- Future profile options may control optimization level, debug information, and diagnostics.
- Profile options must not disable the safety checks specified in [Control Flow](03-control-flow.md#safety-checks-and-build-modes).
- A release build may be faster because the optimizer proves checks unnecessary, not because checks are globally removed.

Users install Nocter by placing the extracted payload at `~/.nocter` or another location, then adding that directory to `PATH`.

Example shell setup:

```sh
export PATH="$HOME/.nocter:$PATH"
```

`NOCTER_HOME` may point to the active Nocter home if the user does not want to rely on the location of the `nocter` executable.

The repository uses `.nocter-arm64-macos/` as the current development output directory for the distributable compiler and standard library. This directory is a generated host package and is not committed to git.
