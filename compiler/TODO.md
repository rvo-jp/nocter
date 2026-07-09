# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Recent committed work:

- Current uncommitted work: `Lower unary bool normal-call expressions`
  - lowers bool-returning normal calls under unary `!`
  - supports `let disabled = !ready()` and `return !ready()`
  - stages the bool call result in a temporary scalar local before materializing `BoolValue::Not`
  - keeps calls directly inside conditions such as `if ready()` reporting `E8006`
  - keeps short-circuit bool expressions with calls, such as `ready() && other()`, disabled until staging can preserve short-circuit evaluation
  - adds IR lowering and CLI run coverage for unary bool normal-call expressions
- `b26f8b7 Lower bool normal calls`
  - adds `Instruction::CallBool` for bool-returning same-file normal calls
  - lowers `let value = ready()` when `ready` returns `bool`
  - emits bool normal calls with the existing framed normal-call sequence, scalar spill/reload, and `i32` argument staging
  - keeps calls directly inside conditions such as `if ready()` reporting `E8006`
  - keeps bool non-tail calls inside compound bool expressions such as `ready() && true` reporting `E8006`
  - adds IR lowering, frame planning, codegen, and CLI run coverage for bool-returning normal-call `let` initializers
- `19c4b92 Stage tail-call arguments`
  - lowers reordered `i32` tail-call arguments such as `return second(b, a)`
  - uses the existing frame argument staging slots for tail calls with arguments, then restores the frame before branching
  - keeps no-argument tail calls frameless
  - keeps nested tail-call arguments such as `return outer(inner())` reporting `E8006`
  - adds IR lowering, frame planning, codegen, and CLI run coverage for reordered tail-call arguments
- `ca2eef1 Lower nested i32 normal-call arguments`
  - lowers normal-call arguments through the same expression-to-value staging path used by additions
  - supports `let value = outer(inner())`, `let value = add(left(), right())`, and `return outer(inner()) + 1`
  - evaluates nested normal-call arguments left to right before the parent `CallI32`
  - keeps nested tail-call arguments such as `return outer(inner())` reporting `E8006`
  - adds IR lowering coverage plus CLI run coverage for nested normal-call arguments
- `9931bf9 Support multiple i32 normal-call result staging`
  - changes expression-to-value lowering to use a shared temporary allocator for each lowered expression
  - evaluates addition operands left to right and stages each normal-call result in a distinct temporary local
  - supports `return left() + right()`, `let value = left() + right()`, and nested additions such as `return (left() + right()) + base`
  - adds IR lowering coverage for temporary/local collision avoidance and CLI run coverage for multi-call additions
- `210d489 Generalize one-call i32 result staging`
  - generalizes one-call `i32` result staging for lowerable additions and grouped forms
- `00c3282 Add normal-call argument staging`
  - extends v0 frame layouts with stack-backed argument staging slots sized to the maximum `CallI32` argument count in a function
  - emits normal-call arguments by evaluating each `i32` argument into a staging slot, then loading `w0` through `w7` from those slots before `bl`
  - allows reordered parameter arguments for source-level normal calls such as `let value = second(b, a)`
  - kept tail-call reordered parameter arguments rejected at that checkpoint
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

- `Lower unary bool normal-call expressions` is pending commit.

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
cargo test --quiet ir::lower::tests::reports_unsupported_nested_i32_tail_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_nested_normal_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_multiple_nested_normal_call_arguments
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_addition_with_nested_normal_call_argument
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
cargo test --quiet --test cli_run run_command_returns_nested_i32_normal_call_argument_exit_code
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

For the tail-call argument staging work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::frame
cargo test --quiet ir::lower::tests::lowers_reordered_tail_call_arguments
cargo test --quiet backend::codegen::tests::generates_i32_tail_call_with_arguments_and_add
cargo test --quiet --test cli_run run_command_returns_reordered_i32_tail_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the bool-returning normal-call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet --no-run
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.
Running test binaries in this sandbox currently hangs before `--list` or `running ...` output, so `cargo test --quiet` and targeted runtime tests could not complete in this environment after the change. An escalation attempt for the targeted lowering test was rejected by the automatic approval reviewer.

For the unary bool normal-call expression work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet --no-run
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.
Runtime test execution remains blocked by the same sandbox test-binary hang described above.

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
2. Consider the next bool-call placement only if it preserves evaluation order; short-circuit expressions with calls need explicit staging semantics.
3. Keep imported calls, aggregates, ownership/drop lowering, nested tail-call arguments, and general condition calls disabled until their lowering rules are designed.
4. Add CLI build/run coverage for any newly buildable source subset.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
