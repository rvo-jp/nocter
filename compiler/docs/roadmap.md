# Compiler Roadmap

This roadmap records implementation order and constraints.
Short-lived handoff notes belong in `../TODO.md`.

## Current Priority

Keep backend v0 narrow while adding guards that prevent malformed IR from reaching code generation.

Recommended next small task:

1. Extend IR lowering's same-file function signature data from return type only to a small signature struct.
2. Include callee return type and lowered parameter count.
3. Keep it deliberately v0-shaped: only same-file, non-generic, `i32` parameters.
4. Reject tail calls whose argument count cannot match the callee before backend codegen sees them.

The frontend already catches normal source mismatches, but IR lowering should not silently construct malformed `Instruction::TailCall`.

## Near-Term Constraints

- Keep non-tail calls unsupported until stack slots, spill/reload, and caller/callee preservation rules exist.
- Do not lower calls inside conditions such as `if enabled() { ... }`.
- Avoid broad `if`, `while`, `loop`, `match`, `var`, reassignment, imports, and aggregate lowering until backend storage and ABI rules are ready.
- Prefer small user-visible build features with integration tests.

## Design Constraints

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 currently has no stack frame, no spill slots, and no ABI-complete non-tail call lowering.
