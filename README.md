<div align="center">
  <img src="./assets/logo.svg" alt="Nocter Logo" width="128">
</div>

# Nocter

Nocter is a statically typed, value-centered systems language for building
native executables from `.nct` source files.

Nocter v0.1.0 is the first narrow v0 release. The current development line is
`v0.2.0-dev`. The implementation target is `arm64-darwin`.

## Why Nocter Exists

Nocter exists because trying a programming language should not require accepting
a large toolchain, a package-manager commitment, or a pile of system-wide
dependencies.

Normal Nocter builds should compile `.nct` source directly to native executable
output without requiring LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or
an external runtime library from the user.

A language should be easy to try and easy to leave. The intended Nocter release
shape is one `.nocter/` directory containing the compiler, metadata, and the
standard library. Installing Nocter should mean unpacking that directory and
placing a `nocter` symlink in a directory already on `PATH`. Uninstalling
Nocter should mean deleting the symlink and that one `.nocter/` directory.

Nocter also comes from frustration with APIs that require users to inspect
private implementation details before they can use the public surface safely.
Public API should be explicit, narrow, and sufficient. Private details should
stay private by default.

Nocter avoids adding many spellings for the same idea. Extra syntax can make a
language feel convenient in isolation, but it increases what humans and AI
assistants must remember. Nocter favors one clear form, source-backed
diagnostics, and formatting rules over a wide menu of equivalent forms.

These motivations become three design pillars:

- **Simplicity**: small distribution shape, low toolchain dependency, and one
  canonical source style.
- **Encapsulation**: public API is the exception; private implementation is the
  default.
- **Foolproof design**: the language should guide ordinary code toward correct
  use and diagnose misuse before backend lowering or runtime execution.

The longer rationale lives in
[Design Principles](spec/00-design-principles.md).

## Language Direction

Nocter is designed around values, modules, explicit contracts, and deterministic
resource handling.

- `struct`, `enum`, `func`, `impl`, and `method` form the value-oriented core.
- Declarations are private unless marked `pub`.
- `interface` is contract-only: it describes public capability without reusable
  code.
- Future `embedding` is composition-only: it will own contained values and
  promote only their public contracts without exposing private internals.
- `let`, `var`, `&T`, and `&+T` make assignment and borrow capability visible.
- `T!` represents recoverable failure.
- `T?` represents absence.
- postfix `?` propagates both `T!` and `T?` early.
- `otherwise` is the single optional fallback form.
- `if` and `match` are value-producing expressions.
- `drop` provides deterministic cleanup instead of a runtime GC.

Nocter deliberately avoids class inheritance, trait-based code reuse, implicit
interface conformance, and hidden runtime machinery in its core direction.

The normative language definition lives in
[spec/](spec/README.md).

## One Directory Install

Nocter release archives are designed to unpack to a single `.nocter/`
directory:

```text
.nocter/
|-- nocter
|-- VERSION
|-- MANIFEST.json
|-- LICENSE
|-- NOTICE
`-- std/
```

Install by placing `.nocter/` somewhere stable, for example under your home
directory, then linking the compiler into a directory already on `PATH`:

```sh
tar -xzf nocter-v0.1.0-arm64-darwin.tar.gz -C "$HOME"
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

Release archives use the `.nocter/` layout above. During repository-local
development, source builds and the local release image remain the way to test
changes before cutting an archive. Development setup lives in
[development/](development/README.md).

For repository-local release testing, the canonical standard-library source is
tracked in `development/std/` and release metadata lives in
`development/packaging/`. Generate a local installation image under
`dist/.nocter/` plus the archive
`dist/nocter-v<version>-arm64-darwin.tar.gz` with:

```sh
./development/compiler/scripts/package-local-release.sh
```

## First Program

Create `main.nct`:

```nct
use std/io.print

func main(): i32! {
    print("Hello from Nocter\n")?
    return 0
}
```

Run it:

```sh
nocter run
```

`nocter run`, `nocter build`, and `nocter check` use `main.nct` when no file is
specified. The entry function is the root-file function named `main`.

Build an executable:

```sh
nocter build -o hello
./hello
```

Check without building:

```sh
nocter check
```

Format a file:

```sh
nocter fmt main.nct
```

## Current Status

The v0.1.0 release and current v0.2.0-dev compiler can parse, check, build,
and run a meaningful v0 subset on `arm64-darwin`. They emit ARM64 Mach-O
executables directly.

The buildable subset is intentionally narrower than the checkable language.
Unsupported runtime forms are part of the post-v0.1.0 roadmap unless promoted;
they should reject with source-backed diagnostics before machine code is
emitted. For the exact implementation boundary, see
[development/docs/implementation-status.md](development/docs/implementation-status.md).

## Learn More

- [Language Specification](spec/README.md): Nocter syntax, type system,
  ownership, standard library, CLI behavior, diagnostics, and tooling contract.
- [Design Principles](spec/00-design-principles.md): the simplicity,
  encapsulation, and foolproof-design rules behind Nocter language decisions.
- [Generics, Interfaces, Embedding, and Methods](spec/08-generics-interfaces-embedding-methods.md):
  the separation between explicit contracts and composition-based reuse.
- [Nocter v0 Contract](spec/00-v0-contract.md): user-facing v0 language
  boundary.
- [Development](development/README.md): Rust bootstrap compiler, tracked
  standard library, release packaging inputs, implementation status, tests, and
  maintenance notes.

## License

Nocter is licensed under the [Apache License, Version 2.0](LICENSE).
