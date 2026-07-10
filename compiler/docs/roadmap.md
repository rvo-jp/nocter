# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Specify the minimal standard-library formatting/allocation API needed before interpolated strings can be lowered.
2. Lower interpolated strings only through explicit standard-library `String` construction and formatting APIs; keep hidden compiler allocation disabled.
3. Consider broader terminal control-flow only after its lowering rules are designed.
4. Keep imported calls, aggregate values, ownership/drop lowering, and general mutable storage disabled until their ABI and storage rules are designed.

## Near-Term Constraints

- Keep broader control-flow disabled until lowering can represent non-terminal effects and joins safely.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Adopted Direction

- Keep the self-contained backend direction. Do not switch to LLVM for the current compiler line.
- Keep safety checks always enabled. Optimizers may remove checks only when they prove the trap condition impossible.
- Treat allocation failure as recoverable failure in ordinary standard-library allocation APIs.
- Model owned `String` as an ordinary standard-library owned value with explicit allocation; the target layout direction is pointer, length, and capacity.
- Do not add a runtime GC.
- Lower generics through monomorphization.
- Prefer static trait dispatch; make dynamic dispatch explicit if it is added later.
- Keep the initial standard library small: primitive trap/unreachable boundaries, process/stderr/syscall wrappers, allocator, owned `String`, and formatting support before collections and file APIs grow.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, framed prologue/epilogue emission, normal-call codegen with conservative scalar spill/reload and argument staging, tail-call argument staging, and source lowering for same-file `i32` normal calls in `let` initializers, arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, `i32` comparison operands, nested normal-call arguments, and nested tail-call arguments plus bool-returning normal calls in `let` initializers, unary-not bool expressions, bool equality/inequality operands, short-circuit bool value expressions, direct terminal-if conditions, terminal-if short-circuit conditions, and short-circuit expressions that combine bool calls with `i32` call comparisons. Lowered `i32` `+`, `-`, `*`, `/`, `%`, `<<`, and `>>` now include the v0 runtime trap checks needed for signed scalar safety.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
