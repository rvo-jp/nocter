# Nocter Compiler Handoff

This file is the short-lived handoff note for the next compiler session.
Durable design belongs in `docs/`; user-facing language rules belong in
`../spec/`.

## Current Repository State

- Branch: `develop`
- Latest known commits:
  - `b9624de Reject relative module paths in expressions`
  - `48b19ea Reject std module paths in expressions`
  - `cd47e74 Reject dotted import paths in parser`
- The current standard-library distribution lives under `../.nocter/std`.
- The active v0 completion definition is `docs/v0-closure.md`.
- Current implementation capability is summarized in
  `docs/implementation-status.md`.

## Current Priority

Keep improving Nocter toward a usable v0 standard-library-driven compiler.

Recommended order:

1. Keep `spec/`, `compiler/docs/v0-closure.md`, and
   `compiler/docs/implementation-status.md` consistent whenever source syntax,
   standard-library API, ABI behavior, or runtime support changes.
2. Close buildability gaps before broadening syntax: accepted source that cannot
   run must fail before IR or backend emission with a source-backed diagnostic.
3. Continue backend and ABI work around aggregates, ownership cleanup, direct
   and indirect calls, enum payload lowering, and supported collection storage.
4. Continue std runtime work only when the public API is stable in
   `spec/11-stdlib-primitives-os.md`.
5. Keep LSP behavior backed by compiler facts. Do not add editor-only semantic
   rules.

## Known Boundaries

- Target support is `arm64-darwin` only.
- `process.env` keeps the future `&str?!` API shape but is not runtime-shipped.
- Bare string interpolation is parsed and typed, but buildability rejects it
  until an explicit allocator source is designed.
- `Vec<T>` supports scalar, `&str`, and current copy-aggregate element storage
  paths. Non-copy aggregate element drop glue, insertion/removal APIs, and
  iteration helpers remain deferred.
- Interface declarations are contract-only. v0 has no interface dispatch,
  generic bounds, trait declarations, or code reuse through interfaces.

## Session Start

Before editing compiler behavior:

```sh
git status --short
git log --oneline -5
```

Read:

- `compiler/README.md`
- `compiler/docs/README.md`
- `compiler/docs/implementation-status.md`
- `compiler/docs/v0-closure.md`
- the relevant `spec/` chapter for the behavior being changed

Do not revert unrelated user changes.

## Verification

Use the narrowest sufficient command set for the change. For broad shared
compiler work, prefer:

```sh
cargo fmt --manifest-path compiler/Cargo.toml --check
cargo test --manifest-path compiler/Cargo.toml --lib
cargo test --manifest-path compiler/Cargo.toml --test cli_build
cargo test --manifest-path compiler/Cargo.toml --test cli_run
cargo test --manifest-path compiler/Cargo.toml --test distributed_home
cargo test --manifest-path compiler/Cargo.toml --test cli_lsp
```

For documentation-only changes, `cargo fmt --manifest-path compiler/Cargo.toml
--check` is usually enough unless examples, CLI contracts, or generated outputs
were changed.
