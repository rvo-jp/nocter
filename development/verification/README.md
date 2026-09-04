# Development Verification

## Responsibility

This directory owns repository-development verification entry points whose intermediate artifacts
must not become persistent repository caches. Release assembly and artifact qualification remain
under [`development/packaging/`](../packaging/).

## Compiler Verification

Run the complete compiler gate from any directory:

```sh
development/verification/verify-compiler.sh
```

The script first verifies the pinned Unicode-data manifest, its mutation guard, and the exact
generated standard-library tables without network access. It then creates one target under `/tmp`,
shares it across formatting, warnings-denied Clippy, workspace tests, feature checking, and Rust
documentation, and removes it on exit. A complete gate therefore cannot add another Cargo hash
generation to `development/compiler/target/`.

Use the workspace target only for focused inner-loop commands. It is a disposable cache; reclaim it
without affecting source or release artifacts with:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml
```

The script intentionally does not clean the workspace target. Verification must not destroy a
developer's active inner-loop cache as a side effect.
