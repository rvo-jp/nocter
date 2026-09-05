# Targets and Distribution

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
- A recognized target name is not the same as an implemented target. A target becomes implemented only when its backend, executable writer, primitive set, and target-gated standard-library boundary exist.

Current target-specific standard-library boundary:

```text
~/.nocter/std/internal/os/index.nct
```

Target-dependent declarations use `#target: "..."` inside stable ordinary directory modules under
`std/`. The current standard package uses boundaries such as:

```text
~/.nocter/std/internal/os/index.nct
~/.nocter/std/internal/path/index.nct
~/.nocter/std/io/index.nct
~/.nocter/std/fs/index.nct
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
        internal/os/index.nct
        process/index.nct
        ptr/index.nct
        string/index.nct
        vec/index.nct
```

The `host` part in the archive name identifies the environment that runs the `nocter` compiler
binary. The current host is `arm64-darwin`, and every archive extracts a `.nocter/` root.

The installed Nocter home contains standard-library directory modules under `std/`. Target-dependent type, helper, and primitive declarations in those modules use `#target: "..."`; ordinary public wrapper functions remain normal functions. The public surface is owned by the checked [standard-library contracts](../../development/std/README.md), while the compiler boundary is specified in [Standard-Library Primitive and OS Boundary](primitives-and-os.md).

Because cross compilation beyond `arm64-darwin` is not implemented, the default active target is the host target. The `arm64-darwin` archive contains the compiler that runs on ARM64 macOS, and `std/internal/os/index.nct` contains the `#target: "arm64-darwin"` primitive boundary for that target.

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
  "schema_version": 2,
  "release": "<version>",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter",
    "sha256": "<lowercase SHA-256>"
  },
  "std": {
    "path": "std",
    "tree_sha256": "<lowercase SHA-256>"
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
- `compiler.sha256` is SHA-256 over the exact bytes of `compiler.path`.
- `std.tree_sha256` binds the complete physical regular tree at `std.path`. The tree hash starts
  with `nocter-regular-tree-v1` followed by a zero byte. Each directory is visited in ascending
  UTF-8 name order. Every descendant contributes its kind (`D` or `F`), normalized `/`-separated
  relative-path byte length as big-endian `u64`, relative UTF-8 path, and content length as
  big-endian `u64`; directories use length zero and files then contribute their exact bytes.
  Symlinks, special files, and non-Unicode paths are invalid.
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
- The running compiler digest must equal `compiler.sha256`, whether the executable is inside or
  outside the selected home.
- The selected home's compiler and standard-library tree must match their manifest digests before
  the installation can supply a toolchain snapshot. A corrupted or partially updated home is
  rejected before package or source analysis.
- The selected standard package must declare package name `std` and the same release as
  `MANIFEST.json`. Package resolution validates this source declaration without duplicating its
  parser in the installation layer.
- The selected Nocter home must contain `VERSION`, `MANIFEST.json`, and `std/`.
- The compiler should report a command-line or Nocter-home error if the selected home is missing required files.

The complete command set, package and file input selection, output naming, and target-option behavior
are specified only by the [Command Line Interface](../tooling/command-line.md). This platform chapter
defines which targets are implemented and the guarantees an implemented target must provide.

Build profile rules:

- Language semantics do not define different safety levels for debug and release builds.
- Profile options must not disable the safety checks specified in [Control Flow](../language/control-flow.md#safety-checks-and-build-modes).
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

## Future Direction

Additional targets require a target backend, executable writer, primitive set, and target-gated
standard-library declarations. Those declarations must remain in the same stable directory modules
used by portable code; ordinary programs must not name CPU instructions, object formats, or OS
syscall details. Cross compilation will use an explicit target option such as `--target x64-linux`.

Additional host archives may use names such as `nocter-v<version>-x64-linux.tar.gz` or
`nocter-v<version>-arm64-linux.tar.gz`, while retaining `.nocter/` as their archive root.

Future build-profile options may control optimization level, debug information, and diagnostics,
but cannot weaken language safety checks.
