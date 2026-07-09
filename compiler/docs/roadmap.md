# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Move back to compiler core work in a fresh session.
2. Review `implementation-status.md`, parser tests, resolver tests, and `analysis/` APIs.
3. Choose a narrow task that improves shared compiler semantics for CLI and LSP together.

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
