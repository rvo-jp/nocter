# Nocter

Nocter is a statically typed, value-centered systems language for building
native executables from `.nct` source files.

The core distribution idea is intentionally simple: install one `.nocter/`
directory, add it to `PATH`, and start writing Nocter. To uninstall, delete that
directory and remove the `PATH` entry. Nocter should be easy to try and easy to
leave.

Nocter's design starts from a practical frustration with existing toolchains and
languages: trying a compiler should not install a stack of unrelated tools,
using an API should not require reading private implementation details, and
writing source should not require remembering several spellings for one idea.
Nocter is built around simplicity, encapsulation, and foolproof design.

Nocter is still pre-v0. The first implementation target is `arm64-darwin`.

## Why Nocter

- **One directory install**: the compiler, metadata, and standard library live
  under `.nocter/`.
- **Easy uninstall**: remove `.nocter/` and delete the shell `PATH` line.
- **Self-contained native output**: normal builds do not require LLVM, `clang`,
  `as`, `ld`, Xcode Command Line Tools, or an external runtime library.
- **Private by default**: public API is explicit, and ordinary use should be
  possible from the public contract alone.
- **Clear source model**: one `.nct` file is one module, imports use `use`, and
  declarations are private unless marked `pub`.
- **Explicit resource handling**: ownership, borrowing, `drop`, `T!` failures,
  and `T?` absence are visible in source.
- **Small standard library first**: `String`, `Vec`, `File`, process APIs,
  formatting, and allocation grow as ordinary Nocter APIs.
- **Tool-friendly syntax**: one canonical style, source-backed diagnostics, JSON
  output, and LSP support are part of the v0 direction.

## Install

Nocter release archives are designed to unpack to a single `.nocter/` directory.
A release archive should contain:

```text
.nocter/
|-- nocter
|-- VERSION
|-- MANIFEST.json
`-- std/
```

Install by placing `.nocter/` somewhere stable, for example under your home
directory:

```sh
tar -xzf nocter-v0-arm64-darwin.tar.gz -C "$HOME"
export PATH="$HOME/.nocter:$PATH"
nocter doctor
```

To make the `PATH` change persistent on zsh:

```sh
printf '\nexport PATH="$HOME/.nocter:$PATH"\n' >> ~/.zshrc
```

The current repository is pre-v0, so source builds are still the main way to
try the compiler before an official release archive exists. Compiler developer
setup lives in [compiler/](compiler/README.md).

## Uninstall

Nocter does not need a package manager-specific uninstall step when installed
as `.nocter/`.

```sh
rm -rf "$HOME/.nocter"
```

Then remove this line from your shell configuration:

```sh
export PATH="$HOME/.nocter:$PATH"
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

## Language Snapshot

Nocter v0 focuses on a small, coherent core:

- `struct`, `enum`, `func`, `impl`, `method`, and contract-only `interface`
- `let` and `var`
- `&T` readonly borrows and `&+T` readwrite borrows
- `String` as an ordinary standard-library owned type
- `Vec<T>` as an explicit standard-library collection
- `T!` for recoverable failure
- `T?` for absence
- postfix `?` for early propagation of both `T!` and `T?`
- `otherwise` for optional fallback
- `if` and `match` as value-producing expressions
- deterministic `drop` instead of a runtime GC

Nocter v0 deliberately does not include class inheritance, trait code reuse,
interface dispatch, embedding, package management, Linux or Windows backends,
or a stable public binary ABI.

## Current Status

The current compiler can parse, check, build, and run a meaningful v0 subset on
`arm64-darwin`. It emits ARM64 Mach-O executables directly.

The buildable subset is still narrower than the checkable language. Unsupported
runtime forms should be rejected with source-backed diagnostics before machine
code is emitted. For the exact implementation boundary, see
[compiler/docs/implementation-status.md](compiler/docs/implementation-status.md).

## Learn More

- [Language Specification](spec/README.md): Nocter syntax, type system,
  ownership, standard library, CLI behavior, diagnostics, and tooling contract.
- [Design Principles](spec/00-design-principles.md): the simplicity,
  encapsulation, and foolproof-design rules behind Nocter language decisions.
- [Compiler Development](compiler/README.md): Rust bootstrap compiler
  architecture, backend work, implementation status, tests, and maintenance
  notes.
- [Nocter v0 Contract](spec/00-v0-contract.md): user-facing v0 language
  boundary.
