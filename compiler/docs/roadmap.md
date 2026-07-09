# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Keep expanding from the narrow source-level scalar normal-call subset already enabled.
2. Consider broader terminal control-flow or nested tail-call arguments only after their lowering rules are designed.
3. Keep imported calls, aggregate values, ownership/drop lowering, and general mutable storage disabled until their ABI and storage rules are designed.

## Near-Term Constraints

- Keep nested tail-call argument lowering conservative until tail calls can consume staged child call results.
- Keep broader control-flow disabled until lowering can represent non-terminal effects and joins safely.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, framed prologue/epilogue emission, normal-call codegen with conservative scalar spill/reload and argument staging, tail-call argument staging, and source lowering for same-file `i32` normal calls in `let` initializers, additions, and nested normal-call arguments plus bool-returning normal calls in `let` initializers, unary-not bool expressions, bool equality/inequality operands, short-circuit bool value expressions, direct terminal-if conditions, and terminal-if short-circuit conditions.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
