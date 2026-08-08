# Targets and Distribution

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Target Model

Nocter currently has one implemented target, while target-specific behavior remains isolated.

Implemented target:

```text
arm64-darwin
```

Target properties:

- CPU architecture: ARM64
- OS: macOS
- executable format: Mach-O
- pointer width: 64-bit
- `usize`: `u64` range
- `isize`: `i64` range

Rules:

- The compiler currently targets only `arm64-darwin`.
- Cross compilation beyond `arm64-darwin` is not supported, but the compiler still models host and target separately.
- The default active target is the host target.
- The language grammar, type system, ownership model, borrow rules, regions, and high-level standard-library APIs should not depend on macOS-specific names.
- Target-specific logic belongs in target backends, primitive lowering, executable writers, and target-gated standard-library declarations.
- The compiler must not depend on external assemblers, linkers, C toolchains, or external runtimes for any target.
- Future targets should be added by introducing new target backends and target-specific standard-library primitive boundaries.
- Future targets must not require ordinary user code to mention CPU instructions, object formats, or OS syscall details.
- Future cross compilation is selected by an explicit target option such as `--target x64-linux`.
- A recognized target name is not the same as an implemented target. A target becomes implemented only when its backend, executable writer, primitive set, and target-gated standard-library boundary exist.

Current target-specific standard-library boundary:

```text
~/.nocter/std/os/index.nct
```

Future target-specific boundaries should keep stable ordinary modules under `std/` and use `#target: "..."` on target-dependent type, helper, and primitive declarations, such as:

```text
~/.nocter/std/os/index.nct
~/.nocter/std/io/index.nct
~/.nocter/std/process/index.nct
```

Target declarations beyond `arm64-darwin` are not currently buildable.

Recognized targets:

```text
arm64-darwin    implemented
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

The downloadable archive name is host-specific, but the archive root and normal user installation directory are host-independent.

Archive name and root:

```text
nocter-v<version>-arm64-darwin.tar.gz

.nocter/
    nocter
    VERSION
    MANIFEST.json
    LICENSE
    NOTICE
    std/
```

The archive root is always `.nocter/`. Users install Nocter by extracting the archive so that `.nocter/` becomes `~/.nocter/`, or by moving the extracted `.nocter/` to another chosen Nocter home, then linking the installed `nocter` binary into a directory already on `PATH`.

The installed layout is:

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    LICENSE
    NOTICE
    std/
        prelude/index.nct
        fmt/index.nct
        io/index.nct
        mem/index.nct
        os/index.nct
        process/index.nct
        ptr/index.nct
        string/index.nct
        vec/index.nct
```

The `host` part in the archive name identifies the environment that runs the `nocter` compiler binary. The current host is `arm64-darwin`. Future downloaded archives may use names such as `nocter-v<version>-x64-linux.tar.gz` or `nocter-v<version>-arm64-linux.tar.gz`, but each archive still extracts a `.nocter/` root.

The installed Nocter home contains standard-library directory modules under `std/`. Target-dependent type, helper, and primitive declarations in those modules use `#target: "..."`; ordinary public wrapper functions remain normal functions. The public standard-library surface is specified in [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md).

Because cross compilation beyond `arm64-darwin` is not implemented, the default active target is the host target. The `arm64-darwin` archive contains the compiler that runs on ARM64 macOS, and `std/os/index.nct` contains the `#target: "arm64-darwin"` primitive boundary for that target.

## Release Metadata

Each Nocter home contains release metadata at its root.

```text
.nocter/
    nocter
    VERSION
    MANIFEST.json
    LICENSE
    NOTICE
    std/
```

`VERSION` is a single UTF-8 text line containing the release version:

```text
<version>
```

`MANIFEST.json` is structured metadata for tools:

```json
{
  "schema": "nocter.manifest",
  "schema_version": 1,
  "release": "<version>",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter"
  },
  "std": {
    "path": "std"
  },
  "license": {
    "id": "Apache-2.0",
    "path": "LICENSE",
    "notice": "NOTICE"
  },
  "implemented_targets": [
    {
      "name": "arm64-darwin",
      "backend": "arm64",
      "executable": "macho",
      "os": "darwin"
    }
  ],
  "archive": {
    "name": "nocter-v<version>-arm64-darwin.tar.gz",
    "root": ".nocter"
  }
}
```

Rules:

- `VERSION`, `MANIFEST.json`, `LICENSE`, and `NOTICE` are required in a
  release archive.
- `VERSION` must match `MANIFEST.json`'s `release`.
- `MANIFEST.json.license.id` is `Apache-2.0`.
- `MANIFEST.json.license.path` and `MANIFEST.json.license.notice` are relative
  to Nocter home.
- `MANIFEST.json.host` identifies the host that runs the bundled `nocter` binary.
- `MANIFEST.json.default_target` is the target used when `--target` is omitted.
- `MANIFEST.json.implemented_targets` lists implemented targets bundled with this Nocter home, not merely reserved target names.
- `compiler.path` is `nocter` and is relative to Nocter home.
- `std.path` is `std` and is relative to Nocter home.
- v1 does not include a compiler checksum. Checksum metadata should be added only after the release pipeline and hash verification rules are designed.
- A release `<version>` uses source tag `v<version>`.
- Its ARM64 macOS asset is `nocter-v<version>-arm64-darwin.tar.gz`.

## Nocter Home Resolution

`nocter` uses an explicit, deterministic Nocter home. It does not silently search unrelated directories.

Resolution order:

1. If `NOCTER_HOME` is set, use it as the active Nocter home.
2. Otherwise, resolve the real path of the running `nocter` executable and use its parent directory.

Rules:

- `NOCTER_HOME` must point to a Nocter home directory, not to `std/`.
- The executable path resolution should resolve symlinks when the host can provide the real executable path.
- `cwd/.nocter` is not searched automatically.
- `~/.nocter` is not searched automatically.
- A symlink such as `/usr/local/bin/nocter -> ~/.nocter/nocter` works naturally because the resolved real executable path still points inside Nocter home.
- Copying `nocter` outside Nocter home is not a normal installation method. If executable-path resolution no longer points into Nocter home, the user must set `NOCTER_HOME`.
- The selected Nocter home must contain `VERSION`, `MANIFEST.json`, and `std/`.
- The compiler should report a command-line or Nocter-home error if the selected home is missing required files.

Future cross compilation adds target-gated standard-library primitive declarations and compiler backends to the installed Nocter home:

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
        os.nct
        io.nct
        process.nct
```

Command-line surface:

```sh
nocter --version
nocter doctor
nocter build
nocter build --root path/to/package
nocter build --executable app -o app
nocter run --executable app
nocter check
nocter check --format json
nocter check app.nct
nocter fmt app.nct
nocter fmt --check app.nct
nocter lsp
nocter build --target arm64-darwin
nocter build --target x64-linux
```

The command-line contract is specified in [Command Line Interface](15-command-line-interface.md).

`build`, `run`, and `check` select the current directory's `nocter.nct` package when no explicit file
is supplied. Package metadata remains Nocter source rather than a second manifest language.

`-o path` sets the executable output path. If `-o` is omitted, the driver derives an output path from the selected executable name or root file stem.

If `--target` is omitted, the compiler uses the host target. The compiler currently emits only `arm64-darwin`. Reserved targets may be recognized by name, but they must produce a not-implemented diagnostic until their backend, executable writer, primitive set, and target standard-library boundary are implemented.

Build profile direction:

- Language semantics do not define different safety levels for debug and release builds.
- Future profile options may control optimization level, debug information, and diagnostics.
- Profile options must not disable the safety checks specified in [Control Flow](03-control-flow.md#safety-checks-and-build-modes).
- A release build may be faster because the optimizer proves checks unnecessary, not because checks are globally removed.

Users install Nocter by placing the extracted `.nocter/` directory at `~/.nocter` or another location, then creating a symlink named `nocter` in a directory already on `PATH`.

Example shell setup:

```sh
ln -s "$HOME/.nocter/nocter" /usr/local/bin/nocter
```

If the target bin directory requires elevated permissions, the user may use
`sudo ln -s ...` or a user-owned directory that is already on `PATH`.
`NOCTER_HOME` may point to the active Nocter home when symlink-based executable
resolution is unavailable or intentionally bypassed.
