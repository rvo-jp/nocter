<div align="center">
  <img src="./docs/assets/logo.svg" alt="Nocter Logo" width="128">
  <p>A self-contained systems language built around simplicity, encapsulation, and foolproof design.</p>
</div>

# Nocter

Nocter is a statically typed, value-centered systems language for building
native executables directly from `.nct` source files.

It favors explicit contracts, private-by-default modules, deterministic
resource cleanup, and one canonical form for each language concept. Nocter uses
`struct`, `enum`, `interface`, `func`, and `method` without class inheritance,
implicit conformance, garbage collection, or hidden runtime machinery.

Recoverable failure and absence are represented by `T!` and `T?`, while
borrowing and mutation capabilities remain visible through `&T` and `&+T`.

See the [language specification](spec/README.md) and
[design principles](spec/00-design-principles.md).

## One Directory Install

[Download nocter-v0.11.0-arm64-darwin.tar.gz](https://github.com/rvo-jp/nocter/releases/download/v0.11.0/nocter-v0.11.0-arm64-darwin.tar.gz)

Nocter compiles source directly to native executables without requiring LLVM,
`clang`, `as`, `ld`, Xcode Command Line Tools, or an external runtime library
from the user.

The compiler, metadata, and standard library live in one `.nocter/` directory.
Installation consists of unpacking that directory and adding a symlink;
uninstallation consists of removing both.

Release archives have this structure:

```text
.nocter/
├── nocter
├── VERSION
├── MANIFEST.json
├── LICENSE
├── NOTICE
└── std/
```

Install by placing `.nocter/` somewhere stable, for example under your home
directory, then linking the compiler into a directory already on `PATH`:

```sh
tar -xzf nocter-v0.11.0-arm64-darwin.tar.gz -C "$HOME"
ln -s "$HOME/.nocter/nocter" /usr/local/bin/nocter
nocter doctor
```

If `/usr/local/bin` requires elevated permissions, use `sudo ln -s ...` or
choose a user-owned directory that is already on `PATH`, such as `~/.local/bin`
when your shell already includes it.

Do not copy the `nocter` binary out of `.nocter/`; the compiler locates its
standard library from the real installed binary path. If symlinks are not
available in your environment, set `NOCTER_HOME` explicitly:

```sh
export NOCTER_HOME="$HOME/.nocter"
```

Uninstalling a `.nocter/` installation is intentionally plain:

```sh
rm /usr/local/bin/nocter
rm -rf "$HOME/.nocter"
```

## First Program

Create `hello.nct`:

```nct
use std/io.print

func main(): i32! {
    print("Hello from Nocter\n")?
    return 0
}
```

Run it directly:

```sh
nocter run hello.nct
```

Build an executable:

```sh
nocter build hello.nct
./hello
```

Check without building:

```sh
nocter check hello.nct
```

Format the source:

```sh
nocter fmt hello.nct
```

Naming the file explicitly selects single-file mode; Nocter does not guess an implicit source
filename. For dependencies, multiple executables, or source files that compose one directory
module, use package mode as demonstrated by
[file-summary](examples/file-summary/index.nct).

## Current Status

The v0.11.0 compiler parses, checks, builds, and runs the supported language on
`arm64-darwin` and emits ARM64 Mach-O executables directly. Unsupported runtime
forms are rejected with source-backed diagnostics before machine code is
emitted.

The [language specification](spec/README.md) describes the current v0.11.0
language. The `v0.11.0` repository tag preserves the exact published compiler,
standard library, specification, and packaging inputs.

## Learn More

- [Examples](examples/README.md): runnable single-file and package examples.
- [Language Specification](spec/README.md): Nocter syntax, type system,
  ownership, standard library, CLI behavior, diagnostics, and tooling contract.
- [Design Principles](spec/00-design-principles.md): the simplicity,
  encapsulation, and foolproof-design rules behind Nocter language decisions.
- [Release Index](releases/README.md): published downloads, supported targets, and version history.
- [Contributor Documentation](development/README.md): development setup,
  compiler architecture, milestone plans, tests, and maintenance policy.

## License

Nocter is licensed under the [Apache License, Version 2.0](LICENSE).
