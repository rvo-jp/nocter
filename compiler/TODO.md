# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Recent committed work:

- `4fdbe41 Add build lowering for bool equality`
  - represents lowerable bool equality/inequality as `BoolValue::BoolComparison`
  - lowers bool equality/inequality when both operands are bool literals, bool locals, or grouped forms of those atoms
  - reports a dedicated `E8008` diagnostic when bool equality/inequality uses lowerable but non-atomic bool operands such as `!ready` or `ready && !blocked`
  - adds ARM64 Darwin codegen for `BoolComparison` using the existing bool register representation and `cmp`/conditional branches
  - adds CLI build/run and IR lowering tests for bool equality/inequality through the native backend path, plus unsupported compound bool equality diagnostics
  - updates implementation status and architecture docs to list bool equality/inequality over literal/local operands in the buildable bool subset
- `d5b1a89 Extract LSP document symbols module`
  - added `driver/lsp/symbols.rs`
  - moved document symbol construction out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the symbols extraction
- `b666f99 Extract LSP completion module`
  - added `driver/lsp/completion.rs`
  - moved keyword and resolved symbol completion item construction out of `driver/lsp/mod.rs`
- `b505f4f Extract LSP hover module`
  - added `driver/lsp/hover.rs`
  - moved hover contents, hover symbol collection, documentation attachment, and resolved-reference hover labels out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the hover extraction
- `6dde4a1 Extract LSP semantic tokens module`
  - added `driver/lsp/semantic.rs`
  - moved semantic token classification and encoding out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the semantic extraction
- `b2643a7 Extract LSP diagnostics module`
  - added `driver/lsp/diagnostics.rs`
  - moved publishDiagnostics payload construction and diagnostic span conversion out of `driver/lsp/mod.rs`
- `deda50e Split LSP foundations and document maintenance policy`
  - moved the LSP server to `driver/lsp/mod.rs`
  - added `driver/lsp/protocol.rs` and `driver/lsp/documents.rs`
  - added `compiler/AGENTS.md` and `docs/maintenance.md`
- `2c73726 Track local symbols in resolver`
  - records local symbols and local identifier targets in resolver output
  - uses local symbols for LSP hover and go-to-definition
- `b318f0d Add basic LSP completions`
  - adds keyword and resolved symbol completions
- `16a13bb Add LSP document symbols`
  - adds document symbol support
- `2dc5785 Add LSP go to definition`
  - adds go-to-definition for resolved symbols

Known unrelated local user changes:

- None observed by `git status --short` at the start of this session.

Do not stage, revert, or modify unrelated files unless the user explicitly asks.

Current uncommitted compiler work:

- Improved build lowering diagnostics for unsupported non-tail calls without expanding backend capabilities:
  - reports a dedicated `E8006` when a function call appears as an i32/bool value instead of in direct tail return position
  - adds IR lowering coverage for i32 and bool non-tail call expressions
  - adds CLI build coverage that verifies the diagnostic and absence of a stale executable
  - updates implementation status to document the dedicated non-tail call diagnostic
- Added the backend v0 normal-call design before implementing stack frames:
  - documents why `bl` requires saving/restoring `x30`
  - specifies the first framed-function shape, fixed spill slots, conservative local spill/reload, argument staging, and IR shape
  - lists the ARM64 encoder instructions required before call lowering
  - updates architecture and roadmap notes to point at `docs/backend-v0.md`
- Added ARM64 encoder support for the first fixed-frame and spill-slot instructions without enabling source-level non-tail call lowering:
  - encodes `sub sp, sp, #imm` and `add sp, sp, #imm`
  - encodes `str`/`ldr x30, [sp, #imm]`
  - encodes `str`/`ldr wN, [sp, #imm]` for scalar local spill/reload slots
  - adds byte-level unit tests for each new instruction family
- Added a backend frame planner without enabling source-level non-tail call lowering:
  - computes fixed 16-byte-aligned frame size
  - reserves the saved `x30` slot at the high end of the frame
  - computes scalar local spill slots below saved `x30`
  - connects the planner to codegen while all current IR functions still plan as frameless
- Added framed-function exit emission without enabling source-level non-tail call lowering:
  - emits `sub sp, sp, #frame_size` and `str x30, [sp, #saved_x30_offset]` in framed prologues
  - emits `ldr x30`, `add sp`, and `ret` for framed returns
  - emits `ldr x30` and `add sp` before framed tail-call branches
  - adds codegen unit tests using an explicit framed layout because current source-lowered IR still has no normal-call instruction
- Added the IR `CallI32` instruction and hand-built IR normal-call codegen coverage without enabling source-level non-tail call lowering:
  - treats `CallI32` as requiring a frame
  - emits conservative scalar local spill/reload around normal calls
  - moves arguments into `w0` through `w7`, emits `bl`, reloads locals, then moves the call result to the destination
  - adds codegen tests for a framed no-argument normal call and a scalar local spill/reload normal call

## Verification Already Run

After the bool equality/inequality lowering work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

All passed.
The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

From repository root:

```sh
git diff --check
```

Passed after the bool equality/inequality lowering work.

For the non-tail call diagnostic work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::reports_unsupported_i32_non_tail_call
cargo test --quiet ir::lower::tests::reports_unsupported_bool_non_tail_call
cargo test --quiet --test cli_build build_command_reports_unsupported_non_tail_call
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the ARM64 encoder frame/spill helper work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the backend frame planner work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the framed-function exit emission work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::codegen::tests::emits_framed
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the IR `CallI32` and hand-built normal-call codegen work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::codegen::tests::generates_framed_i32_normal_call_from_hand_built_ir
cargo test --quiet backend::codegen::tests::normal_i32_call_spills_and_reloads_scalar_locals
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

## First Action In Next Session

1. Run `git status --short`.
2. Review any uncommitted changes before editing.
3. If the user asks for a commit, stage only compiler files unless there are unrelated local changes.

## Next Implementation Direction

The current LSP maintainability pass has reached its planned stopping point:

- `driver/lsp/mod.rs` owns request routing, notification handling, and feature orchestration.
- LSP presentation responsibilities are split across `diagnostics.rs`, `semantic.rs`, `hover.rs`, `definition.rs`, `completion.rs`, `symbols.rs`, and `analysis.rs`.
- Do not add rename, references, formatting integration, or richer type hovers before returning to compiler core work.

Recommended next small task for the next session:

1. Continue compiler core backend work, not LSP-only behavior.
2. Start from `docs/backend-v0.md` normal-call design.
3. Lower the smallest source subset for same-file `i32` normal calls, preferably no-argument or otherwise non-reordered argument cases first.
4. Add CLI build/run coverage for that source subset.
5. Keep imported calls, aggregates, ownership/drop lowering, nested call arguments, and general condition calls disabled.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
