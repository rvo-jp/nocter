# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Adopted user decisions:

- Continue the self-contained backend path; do not switch to LLVM for the current compiler line.
- Keep runtime safety checks always enabled; remove them only when the compiler can prove they cannot trap.
- Treat ordinary allocation failure as recoverable failure, not implicit abort.
- Use an owned `String` direction based on pointer, length, and capacity, implemented as an ordinary standard-library type.
- Do not add a runtime GC.
- Lower generics through monomorphization.
- Prefer static trait dispatch; require an explicit dynamic-dispatch design if it is added later.
- Keep the initial standard library small: trap/unreachable, process/stderr/syscall wrappers, allocator, owned `String`, and formatting support before larger collections or file APIs.

Recommended next implementation order:

1. Design the interpolation allocator source and exact lowering shape around explicit `std/string` construction plus `std/fmt.append_*` calls.
2. Lower interpolated strings only after the needed backend prerequisites exist: imported standard-library calls, aggregate `String` storage, mutable local mutation through `&+String`, and fallible propagation from append calls.
3. Defer broad control flow, unrelated imported calls, aggregate values, general mutable storage, ownership/drop lowering, and optimizer work until their ABI/storage rules are designed.

Recent committed work:

- Current checkpoint: `Specify std string formatting boundary`
  - adds `.nocter/std/fmt.nct` with explicit append APIs for `str`, `String`, `i32`, and `bool`
  - expands `.nocter/std/string.nct` from a placeholder owning type to the initial pointer/length/capacity ABI direction plus `empty`, `with_capacity`, `from_str`, `view`, and `push_str`
  - adds common `error` helper functions in `.nocter/std/mem.nct` for `"std.mem.out_of_memory"` and `"std.mem.invalid_argument"`
  - documents that `std/mem`, `std/string`, and `std/fmt` fail through the built-in `error` payload rather than domain-specific fallible error types
  - keeps interpolated string runtime lowering disabled until an explicit allocator source and backend storage/call prerequisites are implemented
- `Lower i32 shifts`
  - adds IR instructions and lowering for buildable `i32` `<<` and `>>`
  - supports same-file `i32` normal calls inside shift operands through the existing left-to-right temporary staging path
  - emits runtime shift-count traps for negative counts and counts greater than or equal to 32
  - lowers `<<` through ARM64 `lslv` and `>>` through ARM64 `asrv` for signed `i32`
  - adds ARM64 encoder, IR lowering, codegen, CLI build, and CLI run coverage, including negative and too-large count trap paths
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `Add i32 arithmetic overflow traps`
  - emits signed-overflow traps for lowered `i32` addition, subtraction, and multiplication
  - lowers `+` and `-` through ARM64 `adds`/`subs` followed by a `b.vc` guarded `brk #0`
  - lowers `*` through signed 64-bit `smull`, sign-extension comparison, and a `brk #0` when the product does not exactly fit in `i32`
  - adds ARM64 encoder helpers and unit coverage for `adds`, `subs`, `smull`, `sxtw`, 64-bit `cmp`, and `b.vc`
  - adds codegen coverage and CLI run coverage for addition, subtraction, and multiplication overflow trap paths
  - kept shift lowering, imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled at that checkpoint
- `Lower i32 division and remainder`
  - adds ARM64 encoder helpers for `sdiv`, `msub`, and `brk`
  - adds IR lowering and ARM64 codegen for lowerable `i32` division and remainder
  - supports same-file `i32` normal calls inside `/` and `%` arithmetic expressions
  - keeps arithmetic expression evaluation left to right through the existing temporary staging path
  - emits zero-divisor and signed-overflow trap checks before ARM64 `sdiv`
  - adds IR lowering, codegen, CLI build, and CLI run coverage for user-visible `i32` division and remainder, including zero-divisor and signed-overflow trap paths
  - kept imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, broader control-flow, and overflow checks for `+`, `-`, and `*` disabled at that checkpoint
- `Add string interpolation front-end`
  - accepts `${...}` inside single-line and multi-line string source forms while keeping escaped `\${` as literal text
  - adds `InterpolatedString` AST nodes with source-preserving text and expression parts
  - parses interpolation expressions with the normal expression parser over their original byte spans
  - type-checks interpolated string expressions as `String!`
  - accepts interpolation parts of type `str`, `String`, integer, and `bool`
  - reports `E0379` for unsupported interpolation part types such as arrays
  - traverses interpolation expressions during resolution, return/propagation checks, documentation collection, LSP hover collection, and IR call-containment analysis
  - keeps runtime lowering for interpolated string construction disabled until the standard-library formatting/allocation API is finalized
- `Add multi-line string literals`
  - adds shared string literal decoding for single-line and multi-line string literals
  - lexes multi-line `"""..."""` string literals as one `StringLiteral` token without emitting statement newlines for literal content
  - validates multi-line opening newline, closing indentation removal, final UTF-8 after escapes, and `\$`
  - diagnosed unescaped `${` as unimplemented string interpolation instead of accepting it as literal text at that checkpoint
  - updates comment scanning so `//` and `/* */` inside multi-line string literals do not count as comments
  - lowers static fallible failure messages from single-line or multi-line string literals through `return make_error("code", <message>)`
  - kept general `str` values, owned `String`, interpolation parsing/typechecking/lowering, imported calls, aggregate values, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `Lower i32 call arithmetic`
  - adds IR lowering and ARM64 codegen for lowerable `i32` subtraction and multiplication alongside existing addition
  - supports same-file `i32` normal calls inside `+`, `-`, and `*` arithmetic expressions, such as `return answer() * 2 - offset()`
  - keeps arithmetic evaluation left to right through the existing temporary staging path
  - adds IR lowering, ARM64 encoder, CLI build, and CLI run coverage for user-visible `i32` call arithmetic
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `6a9553d Cover i32 comparison short-circuit calls`
  - adds IR lowering and CLI run coverage for short-circuit bool expressions that combine `i32` call comparisons with bool calls
  - covers terminal conditions such as `if answer() == 42 && ready()`
  - covers bool value materialization such as `let matched = answer() == 42 && ready()`
  - confirms the existing short-circuit branch lowering can consume staged `BoolValue::I32Comparison` conditions without additional backend work
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `f3f9df9 Lower i32 call comparisons`
  - lowers same-file `i32` normal calls as `i32` comparison operands
  - supports `if answer() == 42`, `let matched = left() <= right()`, and `return left() < right()`
  - evaluates comparison operands left to right through the existing `i32` expression staging path
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
  - kept unsupported `i32` call expressions such as `return answer() * 2` reporting an IR lowering diagnostic at that checkpoint
  - adds IR lowering and CLI run coverage for `i32` call comparisons
- `3ebddf6 Lower nested tail-call arguments`
  - lowers nested same-file `i32` tail-call arguments such as `return outer(inner())`
  - evaluates nested tail-call arguments left to right through the existing `i32` expression staging path
  - emits child calls before the final `TailCall`, then uses tail-call argument staging for the final branch
  - updates frame planning so `TailCall` argument locals are counted
  - keeps imported calls, bool/aggregate tail-call arguments, ownership/drop lowering, and broader control-flow disabled
  - adds IR lowering, frame planning, and CLI run coverage for nested tail-call arguments
- `1c8a66a Lower bool call comparisons`
  - lowers same-file bool-returning normal calls as atomic bool equality/inequality operands
  - supports `let value = ready() == true`, `return left() != right()`, and `if left() == right()`
  - stages bool call operands left to right before building `BoolValue::BoolComparison`
  - keeps compound bool comparison operands with calls, such as `(ready() && other()) == true`, disabled
  - adds IR lowering and CLI run coverage for bool call comparisons
- `8af4c6b Lower short-circuit bool value calls`
  - lowers same-file bool-returning normal calls in short-circuit bool value expressions
  - supports `let value = ready() && other()` and `return ready() || other()`
  - expands `&&` and `||` to nested `Instruction::If` nodes and materializes `true` or `false` into the destination bool location
  - keeps imported calls, broader control-flow, `var`/reassignment, ownership/drop lowering, and aggregates disabled
  - adds IR lowering and CLI run coverage for short-circuit bool value calls
- `803e63b Lower short-circuit bool condition calls`
  - lowers same-file bool-returning normal calls in terminal `if` `&&` and `||` conditions
  - expands short-circuit conditions to nested `Instruction::If` nodes so the right-hand call is only emitted in the branch where it should execute
  - updates reachable call-target collection to scan nested `Instruction::If` bodies
  - kept short-circuit value expressions with calls, such as `let value = ready() && other()` and `return ready() && other()`, disabled
  - adds IR lowering and CLI run coverage for `&&` and `||` condition calls
- `c8bffa3 Lower bool condition calls`
  - lowers direct same-file bool-returning normal calls in terminal `if` conditions
  - supports `if ready() { ... } else { ... }` and `if !ready() { ... } else { ... }`
  - stages the bool call result in a temporary scalar local before `Instruction::If`
  - kept short-circuit bool expressions with calls, such as `ready() && other()`, disabled until staging can preserve short-circuit evaluation
  - adds IR lowering and CLI run coverage for direct bool normal-call conditions
- `b017d59 Lower unary bool normal-call expressions`
  - lowers bool-returning normal calls under unary `!`
  - supports `let disabled = !ready()` and `return !ready()`
  - stages the bool call result in a temporary scalar local before materializing `BoolValue::Not`
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

- `spec/07-strings-arrays-views-pointers.md`
- `spec/13-lexical-grammar.md`

Do not stage, revert, or modify unrelated files unless the user explicitly asks.

Current uncommitted compiler work:

- None expected after committing `Specify std string formatting boundary`.

## Verification Already Run

For the standard-library string/formatting boundary work, from `compiler/`:

```sh
NOCTER_HOME=/Users/manaberyou/Desktop/nocter/.nocter cargo run --quiet -- check ../.nocter/std/fmt.nct --format json
cargo test --quiet
```

The direct `check` command exits with the expected executable-root diagnostic `E0300` because `std/fmt.nct` is not an executable root file; it produced no import, parse, or type diagnostics after that.

From repository root:

```sh
git diff --check
```

For the i32 shift backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet ir::lower::tests::lowers_entry_i32_shifts_with_normal_calls
cargo test --quiet generates_i32_shift_left_with_count_traps
cargo test --quiet generates_i32_shift_right_with_count_traps
cargo test --quiet --test cli_build build_command_lowers_i32_call_shifts
cargo test --quiet --test cli_run run_command_returns_i32_call_shift_exit_code
cargo test --quiet --test cli_run run_command_traps_i32_negative_shift_count
cargo test --quiet --test cli_run run_command_traps_i32_too_large_shift_count
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. Local assembler output was used once to confirm `lslv` and `asrv` instruction bytes for encoder tests; the compiler implementation still emits those bytes directly and does not depend on an external assembler.

For the i32 arithmetic overflow backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet backend::codegen
cargo test --quiet generates_i32_addition_with_overflow_trap
cargo test --quiet generates_i32_subtraction_with_overflow_trap
cargo test --quiet generates_i32_multiplication_with_overflow_trap
cargo test --quiet --test cli_run run_command_traps_i32_addition_overflow
cargo test --quiet --test cli_run run_command_traps_i32_subtraction_overflow
cargo test --quiet --test cli_run run_command_traps_i32_multiplication_overflow
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. Local assembler output was used once to confirm `smull`, `sxtw`, and 64-bit `cmp` instruction bytes for encoder tests; the compiler implementation still emits those bytes directly and does not depend on an external assembler.

For the i32 division/remainder backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet generates_i32_division_with_safety_traps
cargo test --quiet generates_i32_remainder_with_safety_traps
cargo test --quiet ir::lower::tests::lowers_entry_i32_divide_and_remainder_with_normal_calls
cargo test --quiet --test cli_build build_command_lowers_i32_call_division_and_remainder
cargo test --quiet --test cli_run run_command_returns_i32_call_division_and_remainder_exit_code
cargo test --quiet --test cli_run run_command_traps_i32_division_by_zero
cargo test --quiet --test cli_run run_command_traps_i32_signed_division_overflow
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. One attempted targeted `cargo test` command passed two test names to Cargo and failed argument parsing before being rerun with separate filters.

For the string interpolation front-end work, from `compiler/`:

```sh
cargo fmt
cargo test -q parser::tests::expressions::parses_interpolated_string_expression
cargo test -q typecheck::tests::strings
cargo test -q literals::tests::
cargo test -q lexer::tests::
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the multi-line string literal work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet literals
cargo test --quiet lexer
cargo test --quiet comments
cargo test --quiet parser::tests::expressions::parses_multi_line_string_literal_expression
cargo test --quiet format::tests::formats_multi_line_string_with_comment_markers_stably
cargo test --quiet ir::lower::tests::lowers_fallible_entry_return_make_error_with_multi_line_message
cargo test --quiet --test cli_run run_command_reports_fallible_entry_failure_multi_line_message
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

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

For the bool condition call work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_not_normal_call
cargo test --quiet ir::lower::tests::lowers_bool_if_condition_normal_call
cargo test --quiet --test cli_run run_command_returns_bool_condition_call_exit_code
cargo test --quiet --test cli_run run_command_returns_not_bool_condition_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the short-circuit bool condition call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_and_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_or_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_left_nested_and_normal_calls
cargo test --quiet --test cli_run run_command_returns_and_bool_condition_call_exit_code
cargo test --quiet --test cli_run run_command_returns_or_bool_condition_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the short-circuit bool value call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_and_normal_calls
cargo test --quiet ir::lower::tests::lowers_bool_return_or_normal_calls
cargo test --quiet --test cli_run run_command_returns_and_bool_value_call_exit_code
cargo test --quiet --test cli_run run_command_returns_or_bool_return_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the bool call comparison work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_return_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_normal_call_comparison
cargo test --quiet --test cli_run run_command_returns_bool_call_comparison_let_exit_code
cargo test --quiet --test cli_run run_command_returns_bool_call_comparison_return_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the nested tail-call argument work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_nested_tail_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_multiple_nested_tail_call_arguments
cargo test --quiet backend::frame::tests::tail_call_with_local_argument_counts_argument_local
cargo test --quiet --test cli_run run_command_returns_nested_i32_tail_call_argument_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the i32 call comparison work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_i32_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_i32_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_return_i32_normal_call_comparison
cargo test --quiet --test cli_build build_command_reports_unsupported_i32_call_expression
cargo test --quiet --test cli_run run_command_returns_i32_call_comparison_condition_exit_code
cargo test --quiet --test cli_run run_command_returns_i32_call_comparison_return_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the i32 comparison short-circuit coverage work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_and_i32_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_and_i32_call_comparison
cargo test --quiet --test cli_run run_command_returns_and_i32_call_comparison_condition_exit_code
cargo test --quiet --test cli_run run_command_returns_and_i32_call_comparison_value_exit_code
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

The scalar `i32` backend subset now has runtime safety checks for `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`.

Recommended next small task for the next session:

1. Design the interpolation allocator source and exact lowering shape around explicit `std/string` construction plus `std/fmt.append_*` calls.
2. Add only the backend prerequisites needed by that lowering: imported standard-library calls, aggregate `String` storage, mutable local mutation through `&+String`, and fallible propagation from append calls.
3. Consider broader terminal control-flow only after its lowering rules are designed.
4. Keep unrelated imported calls, aggregates, ownership/drop lowering, general mutable storage, and broader control-flow disabled until their ABI, storage, and join rules are designed.
5. Add CLI build/run coverage for any newly buildable source subset.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
