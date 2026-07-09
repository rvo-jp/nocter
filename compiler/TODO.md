# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Recent committed work:

- `6dde4a1 Extract LSP semantic tokens module`
  - added `driver/lsp/semantic.rs`
  - moved semantic token classification and encoding out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the semantic extraction
- `b2643a7 Extract LSP diagnostics module`
  - added `driver/lsp/diagnostics.rs`
  - moved publishDiagnostics payload construction and diagnostic span conversion out of `driver/lsp/mod.rs`
- `deda50e Split LSP foundations and document maintenance policy`
  - moved the LSP server to `driver/lsp/mod.rs`
  - added `driver/lsp/protocol.rs` and `driver/lsp/documents.rs`
  - added `compiler/AGENTS.md` and `docs/maintenance.md`
- `2c73726 Track local symbols in resolver`
  - records local symbols and local identifier targets in resolver output
  - uses local symbols for LSP hover and go-to-definition
- `b318f0d Add basic LSP completions`
  - adds keyword and resolved symbol completions
- `16a13bb Add LSP document symbols`
  - adds document symbol support
- `2dc5785 Add LSP go to definition`
  - adds go-to-definition for resolved symbols

Known unrelated local user changes:

- None observed by `git status --short` at the start of this session.

Do not stage, revert, or modify unrelated files unless the user explicitly asks.

Current uncommitted compiler work:

- `compiler/src/driver/lsp/hover.rs` now owns hover contents, hover symbol collection, hover documentation attachment, and resolved-reference hover label generation.
- `compiler/src/driver/lsp/mod.rs` routes hover requests to `hover.rs` and reuses hover lookup helpers for definition/completion fallback parsing where needed.
- `compiler/docs/architecture.md`, `compiler/docs/roadmap.md`, and this file were updated to reflect the hover extraction.

## Verification Already Run

After the hover extraction, from `compiler/`:

```sh
cargo fmt
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

All passed.
The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

From repository root:

```sh
git diff --check
```

Passed after the hover extraction.

## First Action In Next Session

1. Run `git status --short`.
2. Review the uncommitted hover extraction.
3. If the user asks for a commit, stage only compiler files unless there are unrelated local changes.

## Next Implementation Direction

Recommended next small task:

1. Continue the LSP maintainability pass.
2. Extract definition support from `driver/lsp/mod.rs` into `driver/lsp/definition.rs`.
3. Keep the extraction behavior-preserving and reuse `hover.rs` only for shared symbol/span lookup where that remains the smallest API.
4. Run `cargo fmt`, `cargo test --quiet`, and `cargo clippy --all-targets --quiet -- -D warnings`.

After that:

1. Extract completion and document symbols one responsibility at a time.
2. Keep compiler semantics in resolver, analysis, and typecheck modules; LSP modules should present those results.
3. Add new editor capabilities only after the existing LSP server structure is easier to maintain.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
