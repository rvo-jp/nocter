use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_usize_condition_exit_code() {
    let project = TempProject::new("cli-run-usize-condition");
    let source = project.write_source(
        "usize_condition.nct",
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 42
    } else {
        return 1
    }
}

func size(): usize {
    return 42
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_usize_range_for_exit_code() {
    let project = TempProject::new("cli-run-usize-range-for");
    let source = project.write_source(
        "usize_range_for.nct",
        r#"func main(): usize {
    let limit: usize = 4
    var total: usize = 0
    for value in 0..<limit {
        total = total + value
    }
    return total
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(6),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_binding_reusing_range_for_name_after_loop() {
    let project = TempProject::new("cli-run-range-for-name-reuse");
    let source = project.write_source(
        "range_for_name_reuse.nct",
        r#"func main(): i32 {
    for value in 0..<2 {
    }
    let value = 5
    return value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(5),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_normal_call_condition_exit_code() {
    let project = TempProject::new("cli-run-bool-normal-call");
    let source = project.write_source(
        "bool_normal_call.nct",
        r#"func main(): i32 {
    let value = ready()
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_not_bool_normal_call_exit_code() {
    let project = TempProject::new("cli-run-not-bool-normal-call");
    let source = project.write_source(
        "not_bool_normal_call.nct",
        r#"func main(): i32 {
    let disabled = !ready()
    if disabled {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-bool-condition-call");
    let source = project.write_source(
        "bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_not_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-not-bool-condition-call");
    let source = project.write_source(
        "not_bool_condition_call.nct",
        r#"func main(): i32 {
    if !ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_and_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-and-bool-condition-call");
    let source = project.write_source(
        "and_bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() && enabled() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func enabled(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_or_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-or-bool-condition-call");
    let source = project.write_source(
        "or_bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() || enabled() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}

func enabled(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_and_bool_value_call_exit_code() {
    let project = TempProject::new("cli-run-and-bool-value-call");
    let source = project.write_source(
        "and_bool_value_call.nct",
        r#"func main(): i32 {
    let value = ready() && enabled()
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func enabled(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_or_bool_return_call_exit_code() {
    let project = TempProject::new("cli-run-or-bool-return-call");
    let source = project.write_source(
        "or_bool_return_call.nct",
        r#"func main(): i32 {
    if enabled() {
        return 42
    } else {
        return 7
    }
}

func enabled(): bool {
    return ready() || fallback()
}

func ready(): bool {
    return false
}

func fallback(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_call_comparison_let_exit_code() {
    let project = TempProject::new("cli-run-bool-call-comparison-let");
    let source = project.write_source(
        "bool_call_comparison_let.nct",
        r#"func main(): i32 {
    let value = ready() == true
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_call_comparison_return_exit_code() {
    let project = TempProject::new("cli-run-bool-call-comparison-return");
    let source = project.write_source(
        "bool_call_comparison_return.nct",
        r#"func main(): i32 {
    if differs() {
        return 42
    } else {
        return 7
    }
}

func differs(): bool {
    return left() != right()
}

func left(): bool {
    return true
}

func right(): bool {
    return false
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_call_comparison_condition_exit_code() {
    let project = TempProject::new("cli-run-i32-call-comparison-condition");
    let source = project.write_source(
        "i32_call_comparison_condition.nct",
        r#"func main(): i32 {
    if answer() == 42 {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_and_i32_call_comparison_condition_exit_code() {
    let project = TempProject::new("cli-run-and-i32-call-comparison-condition");
    let source = project.write_source(
        "and_i32_call_comparison_condition.nct",
        r#"func main(): i32 {
    if answer() == 42 && ready() {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}

func ready(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nonterminal_outer_scalar_assignment_exit_code() {
    let project = TempProject::new("cli-run-nonterminal-outer-scalar-assignment");
    let source = project.write_source(
        "nonterminal_outer_scalar_assignment.nct",
        r#"func main(): i32 {
    var value = 1
    if true {
        value = 2
    }
    while false {
        value = 3
    }
    return value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_loop_break_exit_code() {
    let project = TempProject::new("cli-run-loop-break");
    let source = project.write_source(
        "loop_break.nct",
        r#"func main(): i32 {
    var value = 0
    loop {
        value = 42
        break
    }
    return value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_terminal_loop_exit_code() {
    let project = TempProject::new("cli-run-terminal-loop");
    let source = project.write_source(
        "terminal_loop.nct",
        r#"func main(): i32 {
    loop {
        return 42
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_inequality_exit_code() {
    let project = TempProject::new("cli-run-bool-inequality");
    let source = project.write_source(
        "bool_inequality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let enabled = ready != blocked
    if enabled {
        return 31
    } else {
        return 7
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(31),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_terminal_control_return_expression_exit_code() {
    let project = TempProject::new("cli-run-terminal-control-return-expression");
    let source = project.write_source(
        "terminal_control_return_expression.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    return from_if(1) + from_if_is(Choice.no) + from_match(Choice.maybe)
}

func from_if(value: i32): i32 {
    return if value == 1 {
        10
    } else {
        1
    }
}

func from_if_is(choice: Choice): i32 {
    return if choice is Choice.no {
        20
    } else {
        2
    }
}

func from_match(choice: Choice): i32 {
    return match choice {
        Choice.yes { 3 }
        Choice.no { 4 }
        _ { 12 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_value_if_binding_and_assignment_exit_code() {
    let project = TempProject::new("cli-run-value-if-binding-and-assignment");
    let source = project.write_source(
        "value_if_binding_and_assignment.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let byte: u8 = if true { 5 } else { 1 }
    let size: usize = if byte == 5 { 7 } else { 1 }
    let text: &str = if size == 7 { "Nocter" } else { "Other" }
    let ok: bool = if text == "Nocter" { true } else { false }
    var code = if ok { 10 } else { 1 }
    let choice = Choice.no
    code = if choice is Choice.no { code + 32 } else { 0 }
    return code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_value_control_branch_leading_statements_exit_code() {
    let project = TempProject::new("cli-run-value-control-branch-leading-statements");
    let source = project.write_source(
        "value_control_branch_leading_statements.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let from_if = if true {
        var base = 40
        base = base + 2
        base
    } else {
        var fallback = 0
        fallback = fallback + 1
        fallback
    }
    let choice = Choice.no
    let from_if_is = if choice is Choice.no {
        var value = from_if
        value = value + 0
        value
    } else {
        var fallback = 0
        fallback = fallback + 1
        fallback
    }
    let from_match = match choice {
        Choice.yes {
            var value = 0
            value = value + 1
            value
        }
        Choice.no {
            var value = from_if_is
            value = value + 0
            value
        }
        _ {
            var value = 0
            value = value + 0
            value
        }
    }
    return from_match
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_value_control_call_argument_exit_code() {
    let project = TempProject::new("cli-run-value-control-call-argument");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_control_call_argument.nct",
        r#"use std/string.bytes

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    return score(
        if choice is Choice.no { 5 } else { 1 },
        match choice { Choice.no { 7 } _ { 1 } },
        if choice is Choice.no { true } else { false },
        match choice { Choice.no { "Nocter" } _ { "Other" } },
        match choice { Choice.no { bytes("abc") } _ { bytes("x") } }
    )
}

func score(byte: u8, size: usize, ok: bool, text: &str, data: &[u8]): i32 {
    if byte == 5 && size == 7 && ok && text == "Nocter" && data.len() == 3 {
        42
    } else {
        1
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_value_control_method_call_argument_exit_code() {
    let project = TempProject::new("cli-run-value-control-method-call-argument");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_control_method_call_argument.nct",
        r#"use std/string.bytes

copy struct Checker {
    seed: i32
}

instance Checker {
    method &self.score(byte: u8, size: usize, ok: bool, text: &str, data: &[u8]): i32 {
        if self.seed == 40 && byte == 5 && size == 7 && ok && text == "Nocter" && data.len() == 3 {
            42
        } else {
            1
        }
    }
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let checker = Checker { seed: 40 }
    return checker.score(
        if choice is Choice.no { 5 } else { 1 },
        match choice { Choice.no { 7 } _ { 1 } },
        if choice is Choice.no { true } else { false },
        match choice { Choice.no { "Nocter" } _ { "Other" } },
        match choice { Choice.no { bytes("abc") } _ { bytes("x") } }
    )
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_compound_bool_equality_exit_code() {
    let project = TempProject::new("cli-run-compound-bool-equality");
    let source = project.write_source(
        "compound_bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let unary_same = !ready == blocked
    let logical_same = (ready && !blocked) == ready
    if unary_same && logical_same {
        return 31
    } else {
        return 7
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(31),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_compound_bool_equality_nested_call_exit_code() {
    let project = TempProject::new("cli-run-compound-bool-equality-nested-call");
    let source = project.write_source(
        "compound_bool_equality_nested_call.nct",
        r#"func main(): i32 {
    let both = (ready() && other()) == true
    let neither = !(blocked() || closed()) == true
    if both && neither {
        return 31
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}

func blocked(): bool {
    return false
}

func closed(): bool {
    return false
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(31),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
