# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

Finish the first LSP maintainability pass before adding more editor features.

Recommended next small task:

1. Keep `driver/lsp/` split by stable responsibilities.
2. Extract semantic token classification from `driver/lsp/mod.rs`.
3. Then extract hover, definition, completion, and document symbols only when the new module has a smaller API than the copied context.
4. Reuse resolver and analysis data for editor semantics instead of adding LSP-only semantic logic.

The current LSP feature set is useful enough to exercise in VS Code, but `driver/lsp/mod.rs` is still too large.
Reduce that coupling before adding rename, references, formatting, or richer type hovers.

## Near-Term Constraints

- Keep non-tail calls unsupported until stack slots, spill/reload, and caller/callee preservation rules exist.
- Do not lower calls inside conditions such as `if enabled() { ... }`.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 currently has no stack frame, no spill slots, and no ABI-complete non-tail call lowering.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
