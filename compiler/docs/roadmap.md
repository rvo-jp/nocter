# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.
The fixed definition of `v0 complete` lives in `v0-closure.md`; this roadmap
must not move the completion target without updating that file in the same
commit.

## Current Priority

The first LSP maintainability pass is complete enough to stop here, and compiler core work has moved back to backend v0.

Do not add rename, references, formatting integration, richer type hovers, or more editor-only behavior before returning to compiler core work.
The current LSP feature modules should present compiler analysis results, not grow their own language semantics.

Recommended next implementation order:

1. Use `v0-closure.md` as the source of truth for `ship`, `reject`, and `defer`
   decisions before adding more accepted syntax or backend behavior.
2. Close the frontend audit row first: every parser-accepted form needs
   resolver/typechecker facts or a stable rejection diagnostic before lowering.
3. Add the backend rejection boundary for non-runtime rows so user source does
   not fall through to accidental IR/backend unsupported errors.
4. Continue aggregate ABI and ownership work around field-level state, enum
   payload facts, direct/indirect aggregate edge cases, and drop cleanup.
5. Continue standard-library runtime work around allocator behavior, owned
   `String`, `fmt`, and `process.cwd`; add `Vec`, `args`, and `env` only after
   their public API can remain stable.
6. Keep bare interpolation lowering disabled until an explicit allocator source
   is designed. The explicit `std/mem.page_allocator` +
   `std/string.with_capacity` + `std/fmt.append_str` + `return move out` shape
   now builds and runs through allocation-backed standard-library bodies.

## Near-Term Constraints

- Keep broader control-flow disabled until lowering can represent non-terminal effects and joins safely.
- Avoid broad `if`/`while`/`loop`, `while let`, `match`, non-scalar `var`, compound reassignment, and aggregate forms outside the current supported slot/call-result subset until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.
- Prefer behavior-preserving LSP refactors before adding new LSP capabilities.

## Adopted Direction

- Keep the self-contained backend direction. Do not switch to LLVM for the current compiler line.
- Keep safety checks always enabled. Optimizers may remove checks only when they prove the trap condition impossible.
- Treat allocation failure as recoverable failure in ordinary standard-library allocation APIs.
- Model owned `String` as an ordinary standard-library owned value with explicit allocation; the target layout direction is pointer, length, and capacity. Formatting APIs append into an existing `String` and fail through the built-in `error` payload.
- Do not add a runtime GC.
- Lower generics through monomorphization.
- Traits are deferred after v0. If traits are added later, prefer static
  dispatch and make dynamic dispatch explicit.
- Keep the initial standard library small: primitive trap/unreachable boundaries, process/stderr/syscall wrappers, allocator, owned `String`, and formatting support before collections and file APIs grow.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 has frame layout planning, framed prologue/epilogue emission, normal-call codegen with conservative parameter-word preservation, scalar spill/reload, and typed ABI-word argument staging, tail-call argument staging, and source lowering for same-file and loaded imported calls in the current narrow call subset: `i32` normal calls in `let` initializers, arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, `i32` comparison operands, nested normal-call arguments, nested tail-call arguments, scalar parameters/call arguments, local and readonly parameter scalar borrow arguments plus scalar/aggregate field borrow arguments for normal calls and implicit method receivers, `&str` call arguments, direct `&str` returns, annotated `&str` locals and `&str` normal-call result staging, `usize` locals/returns/normal-call results/comparisons, direct and nested terminal-if returns for entry `i32`/`void` plus non-entry `i32`, `u8`, `usize`, `bool`, `void`, `&str`, and u8 slices, plus bool-returning normal calls in `let` initializers, unary-not bool expressions, bool equality/inequality operands, short-circuit bool value expressions, direct terminal-if conditions, terminal-if short-circuit conditions, short-circuit expressions that combine bool calls with `i32` call comparisons, and short-circuit comparison conditions over scalar aggregate field operands. The same scalar/view/void call subset supports fallible `?`, `!`, and `catch` lowering with the built-in `error.code`/`error.message` payload. Lowered `i32` `+`, `-`, `*`, `/`, `%`, `<<`, and `>>` now include the v0 runtime trap checks needed for signed scalar safety.
- Session handoff and maintenance rules live in `../AGENTS.md` and `maintenance.md`.
