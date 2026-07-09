# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Start the normal-call foundation from `backend-v0.md`, not from source-level call lowering.
2. Lower the smallest source subset for same-file `i32` normal calls.
3. Keep call arguments narrow until argument staging is implemented; start with no-argument or non-reordered argument cases.

## Near-Term Constraints

- Keep source-level non-tail calls unsupported until the `backend-v0.md` frame, spill/reload, and caller/callee preservation plan is implemented.
- Do not lower calls inside conditions such as `if enabled() { ... }`.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, internal framed prologue/epilogue emission, and hand-built IR normal-call codegen with conservative scalar spill/reload, but source-level non-tail call lowering remains disabled.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
