# Targets and Distribution

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

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
- Target-specific logic belongs in target backends, primitive lowering, executable writers, and target-gated standard-library declarations.
- The compiler must not depend on external assemblers, linkers, C toolchains, or external runtimes for any target.
- Future targets should be added by introducing new target backends and target-specific standard-library primitive boundaries.
- Future targets must not require ordinary user code to mention CPU instructions, object formats, or OS syscall details.
- Future cross compilation is selected by an explicit target option such as `--target x64-linux`.
- A recognized target name is not the same as an implemented target. A target becomes implemented only when its backend, executable writer, primitive set, and target-gated standard-library boundary exist.

Current target-specific standard-library boundary:

```text
~/.nocter/std/os.nct
```

Future target-specific boundaries should keep stable ordinary modules under `std/` and use `#target: "..."` on target-dependent type, helper, and primitive declarations, such as:

```text
~/.nocter/std/os.nct
~/.nocter/std/io.nct
~/.nocter/std/process.nct
```

Future target declarations beyond `arm64-darwin` are not part of the initial implementation goal.

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
        prelude.nct
        fmt.nct
        io.nct
        mem.nct
        os.nct
        process.nct
        ptr.nct
        string.nct
        vec.nct
```

The `host` part in the archive name identifies the environment that runs the `nocter` compiler binary. The first host is `arm64-darwin`. Future downloaded archives may use names such as `nocter-v<version>-x64-linux.tar.gz` or `nocter-v<version>-arm64-linux.tar.gz`, but each archive still extracts a `.nocter/` root.

The installed Nocter home contains standard-library source files under `std/`. Target-dependent type, helper, and primitive declarations in those files use `#target: "..."`; ordinary public wrapper functions remain normal functions. The v0.2.0 public standard-library surface is specified in [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md).

Because cross compilation beyond `arm64-darwin` is not part of the initial implementation, the default active target is the host target. For example, the `arm64-darwin` archive contains the compiler that runs on ARM64 macOS, and `std/os.nct` contains the `#target: "arm64-darwin"` primitive boundary for that target.

## Release Metadata

Adopted: each Nocter home contains simple release metadata at its root.

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
    "name": "nocter-v0.1.0-arm64-darwin.tar.gz",
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
- The source repository tag for release `0.1.0` is `v0.1.0`.
- The GitHub Release asset for the first host is `nocter-v0.1.0-arm64-darwin.tar.gz`.

## Nocter Home Resolution

Adopted: `nocter` must use an explicit, deterministic Nocter home. It must not silently search unrelated directories.

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

Initial command-line direction:

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

In v0.4.0 Phase 0, `build`, `run`, and `check` select the current directory's `index.nct` package
when no explicit file is supplied. Package metadata remains Nocter source rather than a second
manifest language. The immutable v0.2.0 single-file boundary is recorded separately.

`-o path` sets the executable output path. If `-o` is omitted, the initial driver may derive an output path from the root file stem.

If `--target` is omitted, the compiler uses the host target. The initial implementation can emit only `arm64-darwin`. Reserved targets may be recognized by name, but they must produce a not-implemented diagnostic until their backend, executable writer, primitive set, and target standard-library overlay are implemented.

Build profile direction:

- The initial language semantics do not define different safety levels for debug and release builds.
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

The repository keeps the canonical standard-library source in
`development/std/` and release metadata templates in `development/packaging/`.
The local release packaging script generates the distributable Nocter home image
at `dist/.nocter/` and the host archive at
`dist/nocter-v<version>-arm64-darwin.tar.gz`.

Rules:

- `development/std/` is edited as source.
- `development/packaging/VERSION` and `development/packaging/MANIFEST.json` are
  edited as release metadata inputs.
- `dist/.nocter/` and `dist/nocter-v<version>-arm64-darwin.tar.gz` are
  generated output and are not committed to git.
