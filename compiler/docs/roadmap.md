# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next small task:

1. Follow `interpolation-lowering.md`: keep bare interpolation lowering disabled until an explicit allocator source is designed, and first make explicit `std/string` construction plus `std/fmt.append_*` calls buildable.
2. Add only the remaining backend prerequisites needed by that explicit path: aggregate reassignment for mutable owned values and owned aggregate return/move body lowering. IR already recognizes ABI-indirect and direct aggregate return signatures plus aggregate borrow parameters, has low-level aggregate slot/call/fallible-call/direct-call/`usize` field-store/copy primitives, and lowers direct aggregate struct literal returns, direct aggregate normal call-result slots, non-entry indirect aggregate struct literal returns with 8-byte integer fields or `std/ptr.from_addr` pointer fields, aggregate struct-literal local slots for those same field shapes, narrow normal/fallible indirect aggregate call result bindings in reserved `let`/`var` slots, aggregate slot borrow arguments, and return-by-name from those slots; aggregate by-value arguments, fallible direct aggregate call-result staging, general pointer expressions, assignment/reassignment, source-level aggregate moves, and aggregate locals outside the supported call-result/struct-literal paths are not buildable yet. Loaded imported scalar calls, scalar parameters/call arguments, local scalar borrow arguments for `&T` and `&+T`, `&str` call arguments, direct `&str` returns, annotated `&str` locals, narrow `&str` normal-call result staging, scalar/view stack-backed `var` plus simple `=` assignment, and ordinary fallible propagation for the current scalar/view/void call subset are already buildable.
3. Consider broader terminal control-flow only after its lowering rules are designed.
4. Keep aggregate values beyond the explicit `String` path, ownership/drop lowering, and general mutable storage disabled until their ABI and storage rules are designed.

## Near-Term Constraints

- Keep broader control-flow disabled until lowering can represent non-terminal effects and joins safely.
- Avoid broad `if`, `while`, `loop`, `match`, non-scalar `var`, compound reassignment, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Adopted Direction

- Keep the self-contained backend direction. Do not switch to LLVM for the current compiler line.
- Keep safety checks always enabled. Optimizers may remove checks only when they prove the trap condition impossible.
- Treat allocation failure as recoverable failure in ordinary standard-library allocation APIs.
- Model owned `String` as an ordinary standard-library owned value with explicit allocation; the target layout direction is pointer, length, and capacity. Formatting APIs append into an existing `String` and fail through the built-in `error` payload.
- Do not add a runtime GC.
- Lower generics through monomorphization.
- Prefer static trait dispatch; make dynamic dispatch explicit if it is added later.
- Keep the initial standard library small: primitive trap/unreachable boundaries, process/stderr/syscall wrappers, allocator, owned `String`, and formatting support before collections and file APIs grow.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, framed prologue/epilogue emission, normal-call codegen with conservative scalar spill/reload and typed ABI-word argument staging, tail-call argument staging, and source lowering for same-file and loaded imported calls in the current narrow call subset: `i32` normal calls in `let` initializers, arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, `i32` comparison operands, nested normal-call arguments, nested tail-call arguments, scalar parameters/call arguments, local scalar borrow arguments for normal calls, `&str` call arguments, direct `&str` returns, annotated `&str` locals and `&str` normal-call result staging, `usize` locals/returns/normal-call results/comparisons, plus bool-returning normal calls in `let` initializers, unary-not bool expressions, bool equality/inequality operands, short-circuit bool value expressions, direct terminal-if conditions, terminal-if short-circuit conditions, and short-circuit expressions that combine bool calls with `i32` call comparisons. The same scalar/view/void call subset supports fallible `?`, `!`, and `catch` lowering with the built-in `error.code`/`error.message` payload. Lowered `i32` `+`, `-`, `*`, `/`, `%`, `<<`, and `>>` now include the v0 runtime trap checks needed for signed scalar safety.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
