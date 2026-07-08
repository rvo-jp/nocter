# Nocter Continuation TODO

This file is the handoff point for the next session. If the user says "TODO.mdを参照して続きの作業を行なって", start here.

## Current Repository State

Recent committed work:

- `9122a58 Test bool terminal if codegen`
  - added backend codegen coverage for `Instruction::If` whose branches return `bool`
- `6f5613d Lower bool terminal if returns`
  - lowered terminal `if` / `else` returning `bool` from non-entry functions
  - updated `compiler/README.md` buildable subset
- `d8fc6b8 Validate tail call return types`
  - added same-file function return signature data to IR lowering
  - rejects direct tail calls whose callee return type is incompatible with the caller return type
- `2056e0b Lower bool tail return calls`
  - allowed direct tail calls from `bool` return positions

Known unrelated local user changes:

- `assets/logo.svg`
- `example.nct`

Do not stage, revert, or modify those unrelated files unless the user explicitly asks.
At handoff time, `git diff --check` still reports `example.nct:168: new blank line at EOF`; treat that as an unrelated user change unless the user asks to clean it up.

## Verification Already Run

From `compiler/`:

```sh
cargo fmt --check
cargo test --quiet
cargo clippy --quiet -- -D warnings
```

All passed after `9122a58`.
The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

From repository root:

```sh
git diff --check
```

This still fails only because of the unrelated `example.nct:168: new blank line at EOF`.

## First Action In Next Session

1. Run `git status --short`.
2. Confirm only the known unrelated local changes are present, plus this `TODO.md` if it has not been committed or removed.
3. Do not touch `assets/logo.svg` or `example.nct` unless the user explicitly asks.
4. Continue backend v0 with the next small guard or buildable feature below.

## Next Implementation Direction

Recommended next small task:

1. Extend IR lowering's same-file function signature data from return type only to a small signature struct.
   - Include callee return type and lowered parameter count.
   - Keep it deliberately v0-shaped: only same-file, non-generic, `i32` parameters.
   - Use it to reject tail calls whose argument count cannot match the callee before backend codegen sees them.
   - The frontend already catches normal source mismatches, but IR lowering should not silently construct malformed `Instruction::TailCall`.

After that:

2. Keep non-tail calls unsupported.
   - Do not lower calls inside conditions such as `if enabled() { ... }`.
   - A normal call can clobber live locals under the current register-only convention.
   - Add stack slots, spill/reload, and caller/callee preservation rules before non-tail calls.

3. Consider the next user-visible build feature only after the guard above.
   - Good candidates are still small terminal control-flow cases.
   - Avoid broad `if`, `while`, `loop`, `switch`, `var`, reassignment, imports, and aggregate lowering until the backend has storage and ABI rules for them.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Backend v0 currently has no stack frame, no spill slots, and no ABI-complete non-tail call lowering.
- Scalar v0 convention:
  - arguments: `w0` through `w7`
  - return: `w0`
  - local scalar bindings: `w9` through `w15`
  - scratch: `w16`, `w17`
