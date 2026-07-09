# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Keep expanding from the narrow source-level `i32` normal-call subset already enabled.
2. Add multiple temporary allocation and evaluation-order rules before allowing expressions with two or more normal calls.
3. Keep imported calls, bool-returning normal calls, aggregate values, nested call arguments, and general condition calls disabled until their lowering rules are designed.

## Near-Term Constraints

- Keep normal-call source lowering narrow until multiple-result staging and evaluation order are explicit.
- Do not lower calls inside conditions such as `if enabled() { ... }`.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, framed prologue/epilogue emission, normal-call codegen with conservative scalar spill/reload and argument staging, and source lowering for a narrow same-file `i32` normal-call subset.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
