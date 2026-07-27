# Nocter Development Handoff

This file is the short-lived handoff note for the next compiler session.
Durable design belongs in `development/docs/`; user-facing language rules belong
in `spec/`.

## Current Repository State

- Branch: `develop`
- Latest known compiler-progress commits:
  - `87f8ee7 Drop legacy root Nocter home entrypoint`
  - `9248e2e Move release image generation to dist`
  - `71f214b Update compiler handoff after value control work`
- The repository root is user-facing. `development/` is the development root.
- The canonical standard-library source lives under `development/std`; local
  release packaging generates `dist/.nocter/std`.
- The active v0 completion definition is `development/docs/v0-closure.md`.
- Current implementation capability is summarized in
  `development/docs/implementation-status.md`.

## Current Priority

Keep improving Nocter toward a usable v0 standard-library-driven compiler.

Recommended order:

1. Keep `spec/`, `development/docs/v0-closure.md`, and
   `development/docs/implementation-status.md` consistent whenever source
   syntax, standard-library API, ABI behavior, or runtime support changes.
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
- `std/ptr.addr`, `std/ptr.from_ref`, and `std/ptr.from_ref_mut` are public
  runtime-shipped address conversions. `from_addr` and raw storage helpers
  remain trusted `pub(nocter)` boundaries; pointer dereference is still
  deferred.
- `Vec<T>` supports scalar, `&str`, and current copy-aggregate element storage
  paths. Non-copy aggregate element drop glue, insertion/removal APIs, and
  iteration helpers remain deferred.
- Interface declarations are contract-only. v0 has no interface dispatch,
  generic bounds, trait declarations, or code reuse through interfaces.

## Recent Notes

- Release packaging layout now separates tracked inputs from generated output:
  `development/std` is the canonical standard-library source,
  `development/packaging` contains release metadata inputs, and
  `development/compiler/scripts/package-local-release.sh` generates
  `dist/.nocter`. Distributed-home tests synthesize a temporary Nocter home from
  those tracked inputs.
- Value-producing `if`, payloadless enum `if is`, and payloadless enum `match`
  branch blocks now lower supported leading bindings, assignments, and
  buildable expression statements before their final value. Buildability
  collects those leading statements and still rejects unsupported branch work
  before IR lowering.
- User-facing parser diagnostics now consistently point to `use` declarations
  instead of describing old import terminology. Block-scope import shadowing
  against parameters and locals is covered by resolver tests.
- CLI coverage now pins bare string interpolation as a source-backed E0435
  buildability rejection before IR lowering, and `fmt` now has coverage for
  block-scope grouped `use` declarations.
- Buildability now rejects trusted `std/ptr.from_addr(0)` as null raw pointer
  construction before IR lowering. The integer literal decoder is shared with
  type checking, so decimal, hex, binary, and underscored zero spellings use the
  same interpretation.
- Buildability signature checks now resolve substituted `TypeExpr` values by
  source file. This matters for std generic helpers specialized with user
  project aggregate types, such as `Vec<Pair>.push`.
- Generic impl method specialization now carries the receiver-derived impl
  substitutions into nested generic calls in the method body.

## Session Start

Before editing compiler behavior:

```sh
git status --short
git log --oneline -5
```

Read:

- `README.md`
- `spec/README.md`
- `development/README.md`
- `development/docs/README.md`
- `development/docs/implementation-status.md`
- `development/docs/v0-closure.md`
- the relevant `spec/` chapter for the behavior being changed

Do not revert unrelated user changes.

## Verification

Use the narrowest sufficient command set for the change. For broad shared
compiler work, prefer:

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
cargo test --manifest-path development/compiler/Cargo.toml --lib
cargo test --manifest-path development/compiler/Cargo.toml --test cli_build
cargo test --manifest-path development/compiler/Cargo.toml --test cli_run
cargo test --manifest-path development/compiler/Cargo.toml --test distributed_home
cargo test --manifest-path development/compiler/Cargo.toml --test cli_lsp
```

For documentation-only changes,
`cargo fmt --manifest-path development/compiler/Cargo.toml --check` is usually
enough unless examples, CLI contracts, or generated outputs were changed.
