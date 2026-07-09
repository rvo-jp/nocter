# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Recent committed work:

- `00c3282 Add normal-call argument staging`
  - extends v0 frame layouts with stack-backed argument staging slots sized to the maximum `CallI32` argument count in a function
  - emits normal-call arguments by evaluating each `i32` argument into a staging slot, then loading `w0` through `w7` from those slots before `bl`
  - allows reordered parameter arguments for source-level normal calls such as `let value = second(b, a)`
  - keeps tail-call reordered parameter arguments rejected because tail calls still use direct sequential argument moves
- `6aca787 Lower source i32 normal-call subset`
  - lowers direct same-file `i32` normal calls to `CallI32` in `let` initializers
  - lowers simple `i32` return additions with one direct normal call by staging the call result in a temporary scalar local
  - keeps imported calls, aggregate args/returns, bool-returning normal calls, ownership/drop lowering, nested call arguments, and general condition calls disabled
- `07c80e9 Add backend normal-call foundation`
  - adds the backend v0 normal-call design, ARM64 frame/spill encoder helpers, fixed frame planning, framed prologue/epilogue emission, and hand-built IR `CallI32` codegen coverage
  - keeps source-level normal-call lowering disabled at that checkpoint
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

- Added multiple normal-call result staging for `i32` additions:
  - changes expression-to-value lowering to use a shared temporary allocator for each lowered expression
  - evaluates addition operands left to right and stages each normal-call result in a distinct temporary local
  - supports `return left() + right()`, `let value = left() + right()`, and nested additions such as `return (left() + right()) + base`
  - adds IR lowering coverage for temporary/local collision avoidance and CLI run coverage for multi-call additions

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

For the source-level normal-call subset work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call_with_arguments
cargo test --quiet ir::lower::tests::lowers_i32_let_initializer_normal_call_with_non_reordered_parameter_arguments
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call_addition
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_local_plus_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_nested_return_addition_with_one_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_with_multiple_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_with_multiple_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_multiple_normal_calls_without_colliding_with_local
cargo test --quiet ir::lower::tests::reports_unsupported_nested_i32_call_argument
cargo test --quiet ir::lower::tests::lowers_reordered_normal_call_arguments
cargo test --quiet ir::lower::tests::reports_unsupported_reordered_tail_call_arguments
cargo test --quiet ir::lower::tests::reports_unsupported_bool_returning_normal_call
cargo test --quiet ir::lower::tests::reports_unsupported_call_in_condition
cargo test --quiet --test cli_build build_command_lowers_i32_normal_call_let_initializer
cargo test --quiet --test cli_build build_command_reports_unsupported_non_tail_call
cargo test --quiet --test cli_run run_command_returns_i32_normal_call_exit_code
cargo test --quiet --test cli_run run_command_returns_reordered_i32_normal_call_exit_code
cargo test --quiet --test cli_run run_command_preserves_local_across_i32_normal_call_addition
cargo test --quiet --test cli_run run_command_returns_multiple_i32_normal_call_addition_exit_code
cargo test --quiet backend::frame
cargo test --quiet backend::codegen::tests::normal_i32_call_spills_and_reloads_scalar_locals
cargo test --quiet backend::codegen::tests::generated_i32_normal_call_stages_reordered_arguments
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
