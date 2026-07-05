# Targets and Distribution

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Target Model

Adopted: Nocter has one initial target, but the compiler architecture should keep target-specific behavior isolated.

Initial target:

```text
arm64-darwin
```

Initial target properties:

- CPU architecture: ARM64
- OS: macOS
- executable format: Mach-O
- pointer width: 64-bit
- `usize`: `u64` range
- `isize`: `i64` range

Rules:

- The initial compiler implementation targets only `arm64-darwin`.
- The initial implementation does not support cross compilation beyond `arm64-darwin`, but the compiler still models host and target separately.
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
~/.nocter/targets/arm64-darwin/std/os/macos.nct
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
arm64-darwin    implemented first
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

Adopted: the downloadable archive name is host-specific, but the archive root and normal user installation directory are host-independent.

The initial archive name and root are:

```text
nocter-v<version>-arm64-darwin.tar.gz

.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
```

The archive root is always `.nocter/`. Users install Nocter by extracting the archive so that `.nocter/` becomes `~/.nocter/`, or by moving the extracted `.nocter/` to another chosen Nocter home.

The installed layout is:

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
        prelude.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
        view.nct
    targets/
        arm64-darwin/
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

The `host` part in the archive name identifies the environment that runs the `nocter` compiler binary. The first host is `arm64-darwin`. Future downloaded archives may use names such as `nocter-v<version>-x64-linux.tar.gz` or `nocter-v<version>-arm64-linux.tar.gz`, but each archive still extracts a `.nocter/` root.

The installed Nocter home contains common standard-library source files and one or more target overlays under `targets/<target>/`.

Because cross compilation beyond `arm64-darwin` is not part of the initial implementation, the default active target is the host target. For example, the `arm64-darwin` archive contains the compiler that runs on ARM64 macOS, and `targets/arm64-darwin/` contains the standard-library primitive boundary for the `arm64-darwin` target.

## Release Metadata

Adopted: each Nocter home contains simple release metadata at its root.

```text
.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
```

`VERSION` is a single UTF-8 text line containing the release version:

```text
0.1.0
```

`MANIFEST.json` is structured metadata for tools:

```json
{
  "schema": "nocter.manifest",
  "schema_version": 1,
  "release": "0.1.0",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter"
  },
  "std": {
    "path": "std"
  },
  "implemented_targets": [
    {
      "name": "arm64-darwin",
      "std_path": "targets/arm64-darwin/std",
      "backend": "arm64",
      "executable": "macho",
      "os": "darwin"
    }
  ],
  "archive": {
    "name": "nocter-v0.1.0-arm64-darwin.tar.gz",
    "root": ".nocter"
  }
}
```

Rules:

- `VERSION` and `MANIFEST.json` are required in a release archive.
- `VERSION` must match `MANIFEST.json`'s `release`.
- `MANIFEST.json.host` identifies the host that runs the bundled `nocter` binary.
- `MANIFEST.json.default_target` is the target used when `--target` is omitted.
- `MANIFEST.json.implemented_targets` lists implemented targets bundled with this Nocter home, not merely reserved target names.
- `compiler.path` is `nocter` and is relative to Nocter home.
- `std.path` is `std` and is relative to Nocter home.
- `implemented_targets[*].std_path` is relative to Nocter home and must exist.
- v1 does not include a compiler checksum. Checksum metadata should be added only after the release pipeline and hash verification rules are designed.
- The source repository tag for release `0.1.0` is `v0.1.0`.
- The GitHub Release asset for the first host is `nocter-v0.1.0-arm64-darwin.tar.gz`.

## Nocter Home Resolution

Adopted: `nocter` must use an explicit, deterministic Nocter home. It must not silently search unrelated directories.

Resolution order:

1. If `NOCTER_HOME` is set, use it as the active Nocter home.
2. Otherwise, resolve the real path of the running `nocter` executable and use its parent directory.

Rules:

- `NOCTER_HOME` must point to a Nocter home directory, not to `std/` or `targets/`.
- The executable path resolution should resolve symlinks when the host can provide the real executable path.
- `cwd/.nocter` is not searched automatically.
- `~/.nocter` is not searched automatically.
- The parent directory of the running `nocter` binary works naturally when the user runs `~/.nocter/nocter` through `PATH`.
- If the user copies or symlinks `nocter` outside Nocter home and executable-path resolution no longer points into Nocter home, the user must set `NOCTER_HOME`.
- The selected Nocter home must contain `VERSION`, `MANIFEST.json`, `std/`, and `targets/`.
- The compiler should report a command-line or Nocter-home error if the selected home is missing required files.

Future cross compilation adds target overlays and compiler backends to the installed Nocter home:

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
        arm64-darwin/
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
nocter --version
nocter doctor
nocter build app.nct
nocter build app.nct -o app
nocter run app.nct
nocter app.nct
nocter check app.nct
nocter check app.nct --format json
nocter fmt app.nct
nocter fmt --check app.nct
nocter lsp
nocter build app.nct --target arm64-darwin
nocter build app.nct --target x64-linux
```

The command-line contract is specified in [Command Line Interface](15-command-line-interface.md).

`build`, `run`, and `check` each take one root `.nct` file. The compiler follows imports from that root file to form the compile unit; it does not read a package manifest in v0.

`-o path` sets the executable output path. If `-o` is omitted, the initial driver may derive an output path from the root file stem.

If `--target` is omitted, the compiler uses the host target. The initial implementation can emit only `arm64-darwin`. Reserved targets may be recognized by name, but they must produce a not-implemented diagnostic until their backend, executable writer, primitive set, and target standard-library overlay are implemented.

Build profile direction:

- The initial language semantics do not define different safety levels for debug and release builds.
- Future profile options may control optimization level, debug information, and diagnostics.
- Profile options must not disable the safety checks specified in [Control Flow](03-control-flow.md#safety-checks-and-build-modes).
- A release build may be faster because the optimizer proves checks unnecessary, not because checks are globally removed.

Users install Nocter by placing the extracted `.nocter/` directory at `~/.nocter` or another location, then adding that directory to `PATH`.

Example shell setup:

```sh
export PATH="$HOME/.nocter:$PATH"
```

`NOCTER_HOME` may point to the active Nocter home if the user does not want to rely on the location of the `nocter` executable.

The repository uses `.nocter/` as the current development output directory for the distributable compiler and standard library. This directory is a generated host package and is not committed to git.
