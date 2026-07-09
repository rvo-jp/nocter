# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Recent committed work:

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

- `assets/logo.svg`

Do not stage, revert, or modify unrelated files unless the user explicitly asks.

Current uncommitted compiler work:

- `compiler/src/driver/lsp.rs` was moved to `compiler/src/driver/lsp/mod.rs`
- `compiler/src/driver/lsp/protocol.rs` now owns JSON-RPC framing and LSP position/range helpers
- `compiler/src/driver/lsp/documents.rs` now owns open document state and URI/path handling
- `compiler/AGENTS.md` and `compiler/docs/maintenance.md` define multi-session maintenance rules

## Verification Already Run

After the LSP split, from `compiler/`:

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

Passed after the LSP split.

## First Action In Next Session

1. Run `git status --short`.
2. Keep `assets/logo.svg` separate unless the user explicitly asks to include it.
3. Review the uncommitted LSP split and maintenance docs.
4. If the user asks for a commit, stage only compiler files and exclude unrelated assets.

## Next Implementation Direction

Recommended next small task:

1. Continue the LSP maintainability pass.
2. Extract diagnostics publishing from `driver/lsp/mod.rs` into `driver/lsp/diagnostics.rs`.
3. Keep the extraction behavior-preserving.
4. Run `cargo fmt`, `cargo test --quiet`, and `cargo clippy --all-targets --quiet -- -D warnings`.

After that:

1. Extract semantic tokens, hover, definition, completion, and document symbols one responsibility at a time.
2. Keep compiler semantics in resolver, analysis, and typecheck modules; LSP modules should present those results.
3. Add new editor capabilities only after the existing LSP server structure is easier to maintain.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
