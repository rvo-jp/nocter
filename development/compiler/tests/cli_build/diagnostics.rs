use super::*;

#[test]
fn build_command_rejects_generic_entry_before_ir_lowering() {
    let project = TempProject::new("cli-build-generic-entry-boundary");
    let source = project.write_source(
        "generic_entry_boundary.nct",
        r#"func main<T>(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0303]"),
        "expected entry signature diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("no type parameters"),
        "expected type parameter entry diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("1 | func main<T>(): i32 {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "frontend should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after frontend diagnostics"
    );
}

#[test]
fn build_command_rejects_mixed_shift_count_before_ir_lowering() {
    let project = TempProject::new("cli-build-mixed-shift-count-diagnostic");
    let source = project.write_source(
        "mixed_shift_count_diagnostic.nct",
        r#"func main(): i32 {
    let value: i32 = 1
    let count: u8 = 1
    return value << count
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert!(
        !output.status.success(),
        "expected build to fail, got stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("error[E0353]"),
        "expected shift diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     return value << count"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "typecheck should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_rejects_nonterminal_if_branch_outer_explicit_drop() {
    let project = TempProject::new("cli-build-nonterminal-if-branch-outer-explicit-drop");
    let source = project.write_source(
        "nonterminal_if_branch_outer_explicit_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        drop file
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit outer aggregate drops inside non-terminal control flow"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("14 |         drop file"),
        "stderr missing drop span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_nonterminal_if_branch_outer_move_binding() {
    let project = TempProject::new("cli-build-nonterminal-if-branch-outer-move-binding");
    let source = project.write_source(
        "nonterminal_if_branch_outer_move_binding.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        var moved = move file
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit outer aggregate moves inside non-terminal control flow"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("14 |         var moved = move file"),
        "stderr missing move span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_nonterminal_while_body_outer_explicit_drop() {
    let project = TempProject::new("cli-build-nonterminal-while-body-outer-explicit-drop");
    let source = project.write_source(
        "nonterminal_while_body_outer_explicit_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        drop file
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit outer aggregate drops inside non-terminal control flow"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("14 |         drop file"),
        "stderr missing drop span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_nonterminal_while_body_outer_move_assignment_before_loop_control() {
    let project =
        TempProject::new("cli-build-nonterminal-while-body-outer-move-assignment-before-control");
    let source = project.write_source(
        "nonterminal_while_body_outer_move_assignment_before_control.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var source = File { fd: 2 }
    while false {
        var target = File { fd: 1 }
        target = move source
        break
        return 1
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit outer aggregate moves inside non-terminal control flow"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("15 |         target = move source"),
        "stderr missing move span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_reports_bool_compound_assignment_compile_diagnostic() {
    let project = TempProject::new("cli-build-bool-compound-assignment-diagnostic");
    let source = project.write_source(
        "bool_compound_assignment_diagnostic.nct",
        r#"func main(): i32 {
    var ready = true
    ready += false
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0437]"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound assignment requires matching integer operands"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("3 |     ready += false"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E0435]"),
        "typecheck should reject before buildability preflight, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_bool_field_compound_assignment_compile_diagnostic() {
    let project = TempProject::new("cli-build-bool-field-compound-assignment-diagnostic");
    let source = project.write_source(
        "bool_field_compound_assignment_diagnostic.nct",
        r#"struct Flag {
    ready: bool
}

func main(): i32 {
    var flag = Flag { ready: true }
    flag.ready += false
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0437]"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("compound assignment requires matching integer operands"),
        "expected compound assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     flag.ready += false"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E0435]"),
        "typecheck should reject before buildability preflight, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_imported_readwrite_borrow_alias_unsupported_argument_before_ir_lowering() {
    let project =
        TempProject::new("cli-build-imported-readwrite-borrow-alias-unsupported-argument");
    project.write_source(
        "borrow_api.nct",
        r#"pub type IntMut = &+i32

pub func touch(value: IntMut): void {
    return
}
"#,
    );
    let source = project.write_source(
        "imported_readwrite_borrow_alias_unsupported_argument.nct",
        r#"use ./borrow_api.touch

copy struct Pair {
    values: [i32; 2]
}

func main(): void {
    var pair = Pair { values: [1, 2] }
    touch(&+pair.values[0])
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("read-write borrow call arguments from unsupported expressions"),
        "expected read-write borrow argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("9 |     touch(&+pair.values[0])"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_compile_diagnostics_without_output() {
    let project = TempProject::new("cli-build-diagnostics");
    let source = project.write_source(
        "bad.nct",
        r#"func main(): i32 {
    return "bad"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0312]"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     return \"bad\""),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("  |            ^^^^^"),
        "expected source underline, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_rejects_ignored_unsupported_method_call_expression_statement_before_ir_lowering() {
    let project = TempProject::new("cli-build-ignored-unsupported-method-call-statement");
    let source = project.write_source(
        "ignored_unsupported_method_call_statement.nct",
        r#"struct Box {
    value: i32
}

impl Box {
    method &+self.borrow_self(): &+Self {
        return self
    }
}

func main(): void {
    var box = Box { value: 1 }
    box.borrow_self()
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("13 |     box.borrow_self()"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "method expression statement should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_self_move_assignment_before_ir_lowering() {
    let project = TempProject::new("cli-build-self-move-assignment");
    let source = project.write_source(
        "self_move_assignment.nct",
        r#"struct File {
    fd: i32
}

func main(): i32 {
    var file = File { fd: 1 }
    file = move file
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0395]"),
        "expected self-move assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     file = move file"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E8008]"),
        "self-move assignment should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_rejects_terminal_if_inside_catch_block_before_ir_lowering() {
    let project = TempProject::new("cli-build-terminal-if-inside-catch-block");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "terminal_if_inside_catch_block.nct",
        r#"use std/error.Error

func fail(): i32! {
    return Error.new("app.fail", "fail")
}

func main(): i32 {
    return fail() catch error {
        if true {
            return 1
        } else {
            return 2
        }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("`catch` blocks outside the v0 runtime subset"),
        "expected catch block construct, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "catch block should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_rejects_nested_otherwise_value_expression_before_ir_lowering() {
    let project = TempProject::new("cli-build-nested-otherwise-value-expression");
    let source = project.write_source(
        "nested_otherwise_value_expression.nct",
        r#"func main(): i32 {
    return use_value((source() otherwise { 1 }) + 2)
}

func use_value(value: i32): i32 {
    return value
}

func source(): i32? {
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "`otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions"
        ),
        "expected otherwise expression construct, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "otherwise expression should be rejected before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_rejects_compound_terminal_if_condition_outer_move() {
    let project = TempProject::new("cli-build-compound-terminal-if-condition-outer-move");
    let source = project.write_source(
        "compound_terminal_if_condition_outer_move.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    if consume(move file) && true {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit aggregate moves in control-flow conditions"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("17 |     if consume(move file) && true {"),
        "stderr missing condition span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_value_if_condition_outer_move() {
    let project = TempProject::new("cli-build-value-if-condition-outer-move");
    let source = project.write_source(
        "value_if_condition_outer_move.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    let code = if consume(move file) {
        0
    } else {
        1
    }
    return code
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit aggregate moves in control-flow conditions"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("17 |     let code = if consume(move file) {"),
        "stderr missing condition span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_nonterminal_while_condition_outer_move() {
    let project = TempProject::new("cli-build-nonterminal-while-condition-outer-move");
    let source = project.write_source(
        "nonterminal_while_condition_outer_move.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    while consume(move file) {
        break
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit aggregate moves in control-flow conditions"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("17 |     while consume(move file) {"),
        "stderr missing condition span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_rejects_imported_nested_alias_condition_aggregate_move_before_ir_lowering() {
    let project = TempProject::new("cli-build-imported-nested-alias-condition-aggregate-move");
    project.write_source(
        "slot_api.nct",
        r#"pub struct Slot {
    pub value: i32
}

type PrivateSlot = Slot
pub type PublicSlot = PrivateSlot

pub func make(): PublicSlot {
    return Slot { value: 1 }
}

pub func consume(slot: PublicSlot): bool {
    return true
}
"#,
    );
    let source = project.write_source(
        "imported_nested_alias_condition_aggregate_move.nct",
        r#"use ./slot_api.PublicSlot
use ./slot_api.make
use ./slot_api.consume

func main(): i32 {
    var slot: PublicSlot = make()
    if consume(move slot) {
        return 1
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit aggregate moves in control-flow conditions"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     if consume(move slot) {"),
        "stderr missing condition span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_does_not_reject_unreachable_range_for_body() {
    let project = TempProject::new("cli-build-unreachable-range-for");
    let source = project.write_source(
        "unreachable_range_for.nct",
        r#"func main(): i32 {
    return 0
}

func unused(): i32 {
    for value in 0..<4 {
        return value
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_unsupported_u64_range_for_before_ir_lowering() {
    let project = TempProject::new("cli-build-u64-range-for-boundary");
    let source = project.write_source(
        "u64_range_for_boundary.nct",
        r#"func main(): i32 {
    return helper(4)
}

func helper(limit: u64): i32 {
    for value in 0..<limit {
        return 1
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("range `for` loops outside i32/usize bounds"),
        "expected range for diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     for value in 0..<limit {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_scalar_local_binding_before_ir_lowering() {
    let project = TempProject::new("cli-build-unsupported-scalar-local-boundary");
    let source = project.write_source(
        "unsupported_scalar_local_boundary.nct",
        r#"func main(): i32 {
    let explicit: u16 = 1
    let inferred = 1 as u16
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("local bindings with unsupported value types"),
        "expected local binding diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     let explicit: u16 = 1"),
        "expected explicit binding source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("3 |     let inferred = 1 as u16"),
        "expected inferred binding source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_imported_alias_storage_only_computation_before_ir_lowering() {
    let project = TempProject::new("cli-build-storage-only-scalar-computation");
    project.write_source("stored.nct", "pub type Stored = u16\n");
    let source = project.write_source(
        "storage_only_scalar_computation.nct",
        r#"use ./stored.Stored

func main(): i32 {
    if (1 as Stored) == (2 as Stored) {
        return 1
    }
    return (1 as Stored) as i32
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr.matches("error[E0435]").count(), 2, "{stderr}");
    assert!(
        stderr.contains("operations on storage-only scalar values"),
        "expected operation diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("conversions from computed storage-only scalar values"),
        "expected conversion diagnostic, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_computed_value_converted_to_storage_only_field_before_ir_lowering() {
    let project = TempProject::new("cli-build-computed-storage-only-field");
    let source = project.write_source(
        "computed_storage_only_field.nct",
        r#"copy struct Stored {
    value: u64
}

func main(): i32 {
    let stored = Stored { value: (runtime_value() + 1) as u64 }
    return 0
}

func runtime_value(): usize {
    return 1
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("computed values converted to storage-only scalar types"),
        "expected storage-only conversion diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     let stored = Stored { value: (runtime_value() + 1) as u64 }"),
        "expected source-backed conversion diagnostic, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_imported_nested_alias_unsupported_scalar_local_binding_before_ir_lowering()
{
    let project = TempProject::new("cli-build-imported-nested-alias-unsupported-scalar-local");
    project.write_source(
        "scalar_aliases.nct",
        r#"type PrivateWide = u16
pub type Wide = PrivateWide
"#,
    );
    let source = project.write_source(
        "imported_nested_alias_unsupported_scalar_local.nct",
        r#"use ./scalar_aliases.Wide

func main(): i32 {
    let explicit: Wide = 1
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("local bindings with unsupported value types"),
        "expected local binding diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let explicit: Wide = 1"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_imported_optional_and_fallible_locals_before_ir_lowering() {
    let project = TempProject::new("cli-build-imported-wrapper-local-boundary");
    project.write_source(
        "wrappers.nct",
        r#"pub type MaybeCount = i32?
pub type Attempt = i32!

pub func maybe(): MaybeCount {
    return none
}

pub func attempt(): Attempt {
    return 1
}
"#,
    );
    let source = project.write_source(
        "imported_wrapper_local_boundary.nct",
        r#"use ./wrappers.{MaybeCount, Attempt, maybe, attempt}

func main(): i32 {
    let explicit_optional: MaybeCount = maybe()
    let inferred_optional = maybe()
    let explicit_fallible: Attempt = attempt()
    let inferred_fallible = attempt()
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr.matches("error[E0435]").count(), 4, "{stderr}");
    assert!(
        stderr.contains("stored optional or fallible local values"),
        "expected wrapper local diagnostic, got:\n{stderr}"
    );
    for line in [4, 5, 6, 7] {
        assert!(
            stderr.contains(&format!("{line} |     let ")),
            "expected source line {line}, got:\n{stderr}"
        );
    }
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_fixed_array_literal_argument_before_ir_lowering() {
    let project = TempProject::new("cli-build-unsupported-fixed-array-literal-argument-boundary");
    let source = project.write_source(
        "unsupported_fixed_array_literal_argument_boundary.nct",
        r#"struct Text {
    value: i32
}

func main(): i32 {
    consume([Text { value: 1 }])
    return 0
}

func consume(texts: [Text; 1]): void {
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function or method parameters outside the v0 runtime ABI subset"),
        "expected unsupported parameter diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("10 | func consume(texts: [Text; 1]): void {"),
        "expected parameter source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("Nocter v0 build cannot lower array literals yet"),
        "expected contextual diagnostics without generic array literal fallback, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_fixed_array_literal_field_before_ir_lowering() {
    let project = TempProject::new("cli-build-unsupported-fixed-array-literal-field-boundary");
    let source = project.write_source(
        "unsupported_fixed_array_literal_field_boundary.nct",
        r#"struct Text {
    value: i32
}

struct Bag {
    texts: [Text; 1]
}

func main(): i32 {
    let bag = Bag { texts: [Text { value: 1 }] }
    return bag.texts[0].value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("field member values outside supported scalar/view or aggregate types"),
        "expected unsupported field member diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("11 |     return bag.texts[0].value"),
        "expected field member source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("Nocter v0 build cannot lower array literals yet"),
        "expected contextual diagnostics without generic array literal fallback, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_scalar_function_signature_before_ir_lowering() {
    let project = TempProject::new("cli-build-unsupported-scalar-signature-boundary");
    let source = project.write_source(
        "unsupported_scalar_signature_boundary.nct",
        r#"func main(): i32 {
    take(1)
    return value() as i32
}

func take(amount: u16): void {
}

func value(): u16 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function or method parameters outside the v0 runtime ABI subset"),
        "expected parameter diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function return types outside the v0 runtime ABI subset"),
        "expected return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 | func take(amount: u16): void {"),
        "expected parameter source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("9 | func value(): u16 {"),
        "expected return source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_scalar_field_member_before_ir_lowering() {
    let project = TempProject::new("cli-build-unsupported-scalar-field-member-boundary");
    let source = project.write_source(
        "unsupported_scalar_field_member_boundary.nct",
        r#"struct Header {
    code: u16
}

func main(): i32 {
    let header = Header { code: 42 }
    return header.code as i32
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("field member values outside supported scalar/view or aggregate types"),
        "expected field member diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 |     return header.code as i32"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_imported_nested_alias_unsupported_scalar_field_member_before_ir_lowering()
{
    let project = TempProject::new("cli-build-imported-nested-alias-unsupported-scalar-field");
    project.write_source(
        "header_api.nct",
        r#"type PrivateCode = u16
pub type Code = PrivateCode

pub struct Header {
    pub code: Code
}
"#,
    );
    let source = project.write_source(
        "imported_nested_alias_unsupported_scalar_field.nct",
        r#"use ./header_api.Header

func main(): i32 {
    let header = Header { code: 42 }
    return header.code as i32
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("field member values outside supported scalar/view or aggregate types"),
        "expected field member diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 |     return header.code as i32"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_non_copy_payload_binding_requires_moved_target() {
    let project = TempProject::new("cli-build-payload-binding-requires-move");
    let source = project.write_source(
        "payload_binding_requires_move.nct",
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    return match result {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0438]"),
        "expected payload ownership diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("return match result {"),
        "expected source-backed target span, got:\n{stderr}"
    );
    assert!(
        stderr.contains("match move result"),
        "expected explicit move help, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after typecheck diagnostics"
    );
}

#[test]
fn build_command_accepts_payload_if_is_owned_direct_drop_binding() {
    let project = TempProject::new("cli-build-payload-if-is-owned-direct-drop-binding");
    let source = project.write_source(
        "payload_if_is_owned_direct_drop_binding.nct",
        r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum AppError {
    missing_path
    open_failed(detail: Detail)
}

func main(): i32 {
    return describe(AppError.missing_path)
}

func describe(error: AppError): i32 {
    if move error is AppError.open_failed(detail) {
        return detail.code
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_owned_payload_move_binding_without_direct_drop() {
    let project = TempProject::new("cli-build-payload-move-binding-without-direct-drop");
    let source = project.write_source(
        "payload_move_binding_without_direct_drop.nct",
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    return match move result {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("owned recursively droppable aggregate types in `match`"),
        "expected payload binding boundary, got:\n{stderr}"
    );
    assert!(
        stderr.contains("Result.ok(detail)"),
        "expected binding source span, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_generic_function() {
    let project = TempProject::new("cli-build-unreachable-generic-function");
    let source = project.write_source(
        "unreachable_generic_function.nct",
        r#"func main(): i32 {
    return 0
}

func identity<T>(value: T): T {
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_reachable_composed_outcome_return() {
    let project = TempProject::new("cli-build-nested-fallible-return-boundary");
    let source = project.write_source(
        "nested_fallible_return_boundary.nct",
        r#"func main(): i32! {
    let value = make_value()? otherwise { return 0 }
    return value
}

func make_value(): i32?! {
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_specialized_composed_outcome_return() {
    let project = TempProject::new("cli-build-specialized-nested-fallible-return-boundary");
    let source = project.write_source(
        "specialized_nested_fallible_return_boundary.nct",
        r#"func main(): i32! {
    let value = make(42)? otherwise { return 0 }
    return value
}

func make<T>(item: T): T?! {
    return item
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_composed_outcome_through_specialized_identity() {
    let project =
        TempProject::new("cli-build-specialized-nested-fallible-type-parameter-return-boundary");
    let source = project.write_source(
        "specialized_nested_fallible_type_parameter_return_boundary.nct",
        r#"func main(): i32! {
    let value = identity(42)? otherwise { return 0 }
    return value
}

func identity<T>(item: T): T?! {
    return item
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_reachable_composed_outcome_method_return() {
    let project = TempProject::new("cli-build-nested-fallible-method-return-boundary");
    let source = project.write_source(
        "nested_fallible_method_return_boundary.nct",
        r#"copy struct Holder {
    pub value: i32
}

impl Holder {
    pub method &self.make_value(): i32?! {
        return none
    }
}

func main(): i32! {
    let holder = Holder { value: 0 }
    let value = holder.make_value()? otherwise { return 0 }
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_specialized_composed_outcome_method_return() {
    let project = TempProject::new("cli-build-specialized-nested-fallible-method-return-boundary");
    let source = project.write_source(
        "specialized_nested_fallible_method_return_boundary.nct",
        r#"copy struct Holder<T> {
    value: T
}

impl<T> Holder<T> {
    pub method &self.make(): T?! {
        return self.value
    }
}

func main(): i32! {
    let holder = Holder<i32> { value: 42 }
    let value = holder.make()? otherwise { return 0 }
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_reachable_composed_outcome_associated_return() {
    let project = TempProject::new("cli-build-nested-fallible-associated-return-boundary");
    let source = project.write_source(
        "nested_fallible_associated_return_boundary.nct",
        r#"copy struct Holder {
    pub value: i32
}

func Holder.make_value(): i32?! {
    return none
}

func main(): i32! {
    let value = Holder.make_value()? otherwise { return 0 }
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_does_not_reject_unreachable_nested_fallible_return() {
    let project = TempProject::new("cli-build-unreachable-nested-fallible-return");
    let source = project.write_source(
        "unreachable_nested_fallible_return.nct",
        r#"func main(): i32 {
    return 0
}

func value(): (i32?)! {
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_does_not_reject_unreachable_nested_fallible_method_return() {
    let project = TempProject::new("cli-build-unreachable-nested-fallible-method-return");
    let source = project.write_source(
        "unreachable_nested_fallible_method_return.nct",
        r#"copy struct Holder {
    pub value: i32
}

impl Holder {
    pub method &self.make_value(): (i32?)! {
        return none
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_std_process_env_check_only_before_ir_lowering() {
    let project = TempProject::new("cli-build-process-env-check-only-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_env_check_only_boundary.nct",
        r#"use std/process.env as lookup

func main(): i32! {
    let value = lookup("HOME")?
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("check-only `std/process.env` calls"),
        "expected env check-only diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let value = lookup(\"HOME\")?"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("nested fallible or optional return types"),
        "std internal return-shape diagnostic should not leak for check-only calls, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_std_process_env_namespace_use_check_only_before_ir_lowering() {
    let project = TempProject::new("cli-build-process-env-namespace-use-check-only-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_env_namespace_use_check_only_boundary.nct",
        r#"use std/process

func main(): i32! {
    let value = process.env("HOME")?
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("check-only `std/process.env` calls"),
        "expected env check-only diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let value = process.env(\"HOME\")?"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("nested fallible or optional return types"),
        "std internal return-shape diagnostic should not leak for check-only calls, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_missing_interpolation_runtime_before_ir_lowering() {
    let project = TempProject::new("cli-build-string-interpolation-boundary");
    let source = project.write_source(
        "string_interpolation_boundary.nct",
        r#"func main(): i32 {
    let text = "value ${1}"
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0440]"),
        "expected interpolation runtime diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("string interpolation runtime is unavailable"),
        "expected missing runtime capability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("2 |     let text = \"value ${1}\""),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_non_copy_aggregate_slice_index_before_ir_lowering() {
    let project = TempProject::new("cli-build-non-copy-aggregate-slice-index-boundary");
    let source = project.write_source(
        "non_copy_aggregate_slice_index_boundary.nct",
        r#"struct Text {
    pub len: i32
}

func read(view: &[Text]): i32 {
    let first = view[0]
    return first.len
}

func main(): i32 {
    return read(source())
}

func source(): &[Text] {
    return source()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("slice indexing outside scalar, `&str`, and copy aggregate elements"),
        "expected slice indexing diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     let first = view[0]"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_non_copy_aggregate_slice_index_assignment_before_ir_lowering() {
    let project = TempProject::new("cli-build-non-copy-aggregate-slice-index-assignment-boundary");
    let source = project.write_source(
        "non_copy_aggregate_slice_index_assignment_boundary.nct",
        r#"struct Text {
    pub len: i32
}

func replace(view: &+[Text]): void {
    view[0] = Text { len: 42 }
    return
}

func main(): i32 {
    replace(source())
    return 0
}

func source(): &+[Text] {
    return source()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("index assignment targets outside supported slice values"),
        "expected slice assignment diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("6 |     view[0] = Text { len: 42 }"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_null_from_addr_before_ir_lowering() {
    let project = TempProject::new("cli-build-null-from-addr-boundary");
    project.write_nocter_home_file(
        "std/ptr.nct",
        r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
    );
    project.write_nocter_home_file(
        "std/buffer.nct",
        r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func buffer(): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(0), 0)
}
"#,
    );
    let source = project.write_source(
        "null_from_addr_boundary.nct",
        r#"use std/buffer.buffer

func main(): i32 {
    let bytes = buffer()
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("null raw pointer construction"),
        "expected null pointer diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 |     return slice_from_raw_parts_mut(from_addr(0), 0)"),
        "expected std source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_reports_cast_null_from_addr_before_ir_lowering() {
    let project = TempProject::new("cli-build-cast-null-from-addr-boundary");
    project.write_nocter_home_file(
        "std/ptr.nct",
        r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
    );
    project.write_nocter_home_file(
        "std/buffer.nct",
        r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func buffer(): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(0 as usize), 0)
}
"#,
    );
    let source = project.write_source(
        "cast_null_from_addr_boundary.nct",
        r#"use std/buffer.buffer

func main(): i32 {
    let bytes = buffer()
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("null raw pointer construction"),
        "expected null pointer diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("5 |     return slice_from_raw_parts_mut(from_addr(0 as usize), 0)"),
        "expected std source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_array_literal() {
    let project = TempProject::new("cli-build-unreachable-array-literal");
    let source = project.write_source(
        "unreachable_array_literal.nct",
        r#"func main(): i32 {
    return 0
}

func header(): [u8; 2] {
    return [1, 2]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_skips_unreachable_entry_tail_after_return() {
    let project = TempProject::new("cli-build-unreachable-entry-tail-after-return");
    let source = project.write_source(
        "unreachable_entry_tail_after_return.nct",
        r#"func main(): i32 {
    return 0
    let header: [u8; 2] = [1, 2]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn build_command_skips_unreachable_entry_tail_after_exhaustive_match_statement() {
    let project =
        TempProject::new("cli-build-unreachable-entry-tail-after-exhaustive-match-statement");
    let source = project.write_source(
        "unreachable_entry_tail_after_exhaustive_match_statement.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes {
            return 0
        }

        Choice.no {
            return 1
        }
    }
    let stored: u16 = 0 as u16
    return 2
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn build_command_rejects_nonterminal_match_arm_outer_explicit_drop() {
    let project = TempProject::new("cli-build-nonterminal-match-arm-outer-explicit-drop");
    let source = project.write_source(
        "nonterminal_match_arm_outer_explicit_drop.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

enum Choice {
    yes
    no
}

func main(): i32 {
    var file = File { fd: 1 }
    let choice = Choice.yes
    match choice {
        Choice.yes {
            drop file
        }
        _ {
        }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0435]"),
        "stderr missing E0435:\n{stderr}"
    );
    assert!(
        stderr.contains("explicit outer aggregate drops inside non-terminal control flow"),
        "stderr missing unsupported construct:\n{stderr}"
    );
    assert!(
        stderr.contains("21 |             drop file"),
        "stderr missing drop span:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "stderr leaked IR diagnostic:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "unexpected executable at {}",
        executable.display()
    );
}

#[test]
fn build_command_accepts_payload_match_expression_owned_direct_drop_binding() {
    let project = TempProject::new("cli-build-payload-match-expression-owned-direct-drop-binding");
    let source = project.write_source(
        "payload_match_expression_owned_direct_drop_binding.nct",
        r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 10 })
    return match move result {
        Result.ok(value) { value.code }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_static_error_payload_helper_with_argument_before_ir_lowering() {
    let project = TempProject::new("cli-build-static-error-payload-helper-argument-boundary");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "static_error_payload_helper_argument_boundary.nct",
        r#"use std/error.Error

func main(): i32! {
    return app_failed(dynamic_message())
}

func app_failed(message: &str): error {
    return Error.new("app.failed", "failed")
}

func dynamic_message(): &str {
    return "ignored"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function return types outside the v0 runtime ABI subset"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 | func app_failed(message: &str): error {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_imported_error_constructor_with_non_str_payload_before_ir_lowering() {
    let project = TempProject::new("cli-build-imported-error-constructor-non-str-boundary");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    project.write_source(
        "bad_error.nct",
        r#"use std/error.Error

pub func app_failed(code: i32, message: i32): error {
    return Error.new("app.failed", "failed")
}
"#,
    );
    let source = project.write_source(
        "imported_error_constructor_non_str_boundary.nct",
        r#"use ./bad_error.app_failed

func main(): i32! {
    return app_failed(1, 2)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function return types outside the v0 runtime ABI subset"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("3 | pub func app_failed(code: i32, message: i32): error {"),
        "expected source line from imported helper, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_error_return_method_helper_before_ir_lowering() {
    let project = TempProject::new("cli-build-error-return-method-helper-boundary");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "error_return_method_helper_boundary.nct",
        r#"use std/error.Error

copy struct Holder {
    value: i32
}

impl Holder {
    method &self.app_failed(): error {
        return Error.new("app.failed", "failed")
    }
}

func main(): i32! {
    let holder = Holder { value: 0 }
    return holder.app_failed()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("method return types outside the v0 runtime ABI subset"),
        "expected method return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("8 |     method &self.app_failed(): error {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_dynamic_error_return_helper_before_ir_lowering() {
    let project = TempProject::new("cli-build-dynamic-error-return-helper-boundary");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "dynamic_error_return_helper_boundary.nct",
        r#"use std/error.Error

func main(): i32! {
    return app_failed(dynamic_message())
}

func app_failed(message: &str): error {
    return Error.new("app.failed", message)
}

func dynamic_message(): &str {
    return "failed"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]"),
        "expected v0 buildability diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("function return types outside the v0 runtime ABI subset"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("7 | func app_failed(message: &str): error {"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[test]
fn build_command_does_not_reject_unreachable_dynamic_failure_payload() {
    let project = TempProject::new("cli-build-unreachable-dynamic-failure-payload");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "unreachable_dynamic_failure_payload.nct",
        r#"use std/error.Error

func main(): i32 {
    return 0
}

func unused(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_rejects_use_after_moved_non_copy_aggregate() {
    let project = TempProject::new("cli-build-reject-use-after-moved-non-copy-aggregate");
    let source = project.write_source(
        "use_after_moved_non_copy_aggregate.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair { a: 10, b: 20, c: 7, d: 5 }
    let total = check(move pair)
    return total + pair.a
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0385]"),
        "expected ownership diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("because it was moved"),
        "expected moved-state diagnostic, got:\n{stderr}"
    );
}

#[test]
fn build_command_rejects_implicit_non_copy_aggregate_argument() {
    let project = TempProject::new("cli-build-reject-implicit-non-copy-aggregate-argument");
    let source = project.write_source(
        "implicit_non_copy_aggregate_argument.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair { a: 10, b: 20, c: 7, d: 5 }
    return check(pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0392]"),
        "expected aggregate argument typecheck diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot implicitly copy non-copy struct `Pair` from `pair`"),
        "expected non-copy aggregate argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|     return check(pair)"),
        "expected source line for aggregate argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|                  ^^^^"),
        "expected source underline for aggregate argument diagnostic, got:\n{stderr}"
    );
}

#[test]
fn build_command_rejects_implicit_non_copy_generic_copy_struct_argument() {
    let project =
        TempProject::new("cli-build-reject-implicit-non-copy-generic-copy-struct-argument");
    let source = project.write_source(
        "implicit_non_copy_generic_copy_struct_argument.nct",
        r#"struct Text {
    len: i32
}

copy struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<Text> { value: Text { len: 42 } }
    return read(box)
}

func read(box: Box<Text>): i32 {
    return box.value.len
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0392]"),
        "expected aggregate argument typecheck diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "cannot implicitly copy move-only copy-struct instantiation `Box<Text>` from `box`"
        ),
        "expected generic copy struct argument diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|     return read(box)"),
        "expected source line for generic copy struct argument diagnostic, got:\n{stderr}"
    );
}

#[test]
fn build_command_rejects_implicit_non_copy_aggregate_return() {
    let project = TempProject::new("cli-build-reject-implicit-non-copy-aggregate-return");
    let source = project.write_source(
        "implicit_non_copy_aggregate_return.nct",
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    return make().len
}

func make(): Text {
    let text = Text { start: 1, len: 42, capacity: 99 }
    return text
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("error[E0393]"),
        "expected aggregate return typecheck diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot implicitly copy non-copy struct `Text` from `text`"),
        "expected non-copy aggregate return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|     return text"),
        "expected source line for aggregate return diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("|            ^^^^"),
        "expected source underline for aggregate return diagnostic, got:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_fallible_entry_failure_reports_stderr() {
    let project = TempProject::new("cli-build-run-fallible-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let output = Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.failed: failed\n");
}
