use super::*;

#[test]
fn build_command_lowers_usize_let_and_condition() {
    let project = TempProject::new("cli-build-usize-let-condition");
    let source = project.write_source(
        "usize_condition.nct",
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 0
    } else {
        return 1
    }
}

func size(): usize {
    return 42
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_usize_terminal_if_function() {
    let project = TempProject::new("cli-build-usize-terminal-if-function");
    let source = project.write_source(
        "usize_terminal_if_function.nct",
        r#"func main(): i32 {
    let value: usize = choose(true)
    if value == 7 {
        return 0
    } else {
        return 1
    }
}

func choose(flag: bool): usize {
    if flag {
        return 7
    } else {
        return 9
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
fn build_command_lowers_void_terminal_if_function() {
    let project = TempProject::new("cli-build-void-terminal-if-function");
    let source = project.write_source(
        "void_terminal_if_function.nct",
        r#"func main(): i32 {
    run(true)
    return 0
}

func run(flag: bool): void {
    if flag {
        return
    } else {
        return
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
fn build_command_lowers_trailing_void_if_before_implicit_return() {
    let project = TempProject::new("cli-build-trailing-void-if-implicit-return");
    let source = project.write_source(
        "trailing_void_if_implicit_return.nct",
        r#"func main(): void {
    run(true)
}

func run(flag: bool): void {
    if flag {
        effect()
    }
}

func effect(): void {
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if() {
    let project = TempProject::new("cli-build-terminal-if");
    let source = project.write_source(
        "terminal_if.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 2
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
fn build_command_lowers_nested_terminal_if() {
    let project = TempProject::new("cli-build-nested-terminal-if");
    let source = project.write_source(
        "nested_terminal_if.nct",
        r#"func main(): i32 {
    if true {
        if false {
            return 1
        } else {
            return 0
        }
    } else {
        return 2
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
fn build_command_lowers_terminal_nested_if_in_nonterminal_while_body() {
    let project = TempProject::new("cli-build-terminal-nested-if-in-nonterminal-while-body");
    let source = project.write_source(
        "terminal_nested_if_in_nonterminal_while_body.nct",
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        if true {
            break
        } else {
            continue
        }
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
fn build_command_lowers_terminal_nested_if_in_nonterminal_loop_body() {
    let project = TempProject::new("cli-build-terminal-nested-if-in-nonterminal-loop-body");
    let source = project.write_source(
        "terminal_nested_if_in_nonterminal_loop_body.nct",
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    loop {
        var file = File { fd: 1 }
        if true {
            break
        } else {
            continue
        }
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
fn build_command_lowers_return_in_nonterminal_while_body() {
    let project = TempProject::new("cli-build-return-in-nonterminal-while-body");
    let source = project.write_source(
        "return_in_nonterminal_while_body.nct",
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        return 7
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
fn build_command_lowers_terminal_if_branch_void_call() {
    let project = TempProject::new("cli-build-terminal-if-branch-void-call");
    let source = project.write_source(
        "terminal_if_branch_void_call.nct",
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        touch(&+file)
        return 0
    } else {
        return 1
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_bool_local() {
    let project = TempProject::new("cli-build-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local.nct",
        r#"func main(): i32 {
    let enabled = true
    if enabled {
        return 0
    } else {
        return 1
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
fn build_command_lowers_terminal_if_bool_logical() {
    let project = TempProject::new("cli-build-terminal-if-bool-logical");
    let source = project.write_source(
        "terminal_if_bool_logical.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if ready && !blocked {
        return 0
    } else {
        return 1
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
fn build_command_lowers_bool_equality() {
    let project = TempProject::new("cli-build-bool-equality");
    let source = project.write_source(
        "bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = ready == blocked
    if same {
        return 1
    } else {
        return 0
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
fn build_command_lowers_compound_bool_equality() {
    let project = TempProject::new("cli-build-compound-bool-equality");
    let source = project.write_source(
        "compound_bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
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
fn build_command_lowers_function_leading_compound_bool_equality() {
    let project = TempProject::new("cli-build-function-leading-binding-span");
    let source = project.write_source(
        "function_leading_binding_span.nct",
        r#"func main(): i32 {
    return helper()
}

func helper(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
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
fn build_command_lowers_compound_bool_equality_in_nonterminal_if_binding() {
    let project = TempProject::new("cli-build-nonterminal-if-binding-span");
    let source = project.write_source(
        "nonterminal_if_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    if ready {
        let ok = true
        let same = !ok == ready
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
fn build_command_lowers_compound_bool_equality_in_nonterminal_while_binding() {
    let project = TempProject::new("cli-build-nonterminal-while-binding-span");
    let source = project.write_source(
        "nonterminal_while_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    while ready {
        let ok = true
        let same = !ok == ready
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
fn build_command_lowers_compound_bool_equality_in_terminal_if_branch_binding() {
    let project = TempProject::new("cli-build-terminal-if-branch-binding-span");
    let source = project.write_source(
        "terminal_if_branch_binding_span.nct",
        r#"func main(): i32 {
    let ready = true
    if ready {
        let ok = true
        let same = !ok == ready
        return 1
    } else {
        return 0
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
fn build_command_lowers_compound_bool_equality_condition() {
    let project = TempProject::new("cli-build-compound-bool-equality-condition");
    let source = project.write_source(
        "compound_bool_equality_condition.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if !ready == blocked {
        return 1
    } else {
        return 0
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
fn build_command_lowers_compound_bool_equality_nested_call_operand() {
    let project = TempProject::new("cli-build-compound-bool-equality-call-boundary");
    let source = project.write_source(
        "compound_bool_equality_call_boundary.nct",
        r#"func main(): i32 {
    if (ready() && other()) == true {
        return 0
    } else {
        return 1
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        executable.exists(),
        "build should leave an executable for nested call bool equality"
    );
}

#[test]
fn build_command_lowers_compound_bool_equality_in_nonterminal_while_condition() {
    let project = TempProject::new("cli-build-nonterminal-while-condition-span");
    let source = project.write_source(
        "nonterminal_while_condition_span.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    while !ready == blocked {
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
fn build_command_lowers_terminal_if_i32_equality() {
    let project = TempProject::new("cli-build-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return 0
    } else {
        return 1
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
fn build_command_lowers_terminal_if_i32_inequality() {
    let project = TempProject::new("cli-build-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return 0
    } else {
        return 1
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
fn build_command_lowers_terminal_if_i32_less_equal() {
    let project = TempProject::new("cli-build-terminal-if-less-equal");
    let source = project.write_source(
        "terminal_if_less_equal.nct",
        r#"func main(): i32 {
    let value = 42
    if value <= 42 {
        return 0
    } else {
        return 1
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
fn build_command_lowers_reachable_i32_range_for() {
    let project = TempProject::new("cli-build-range-for");
    let source = project.write_source(
        "range_for.nct",
        r#"func main(): i32 {
    return helper()
}

func helper(): i32 {
    var total = 0
    for value in 0..<4 {
        total = total + value
    }

    return total
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(6));
}

#[test]
fn build_command_accepts_value_if_branch_leading_bindings() {
    let project = TempProject::new("cli-build-value-if-branch-leading-bindings");
    let source = project.write_source(
        "value_if_branch_leading_bindings.nct",
        r#"func main(): i32 {
    let answer = if true {
        let base = 40
        base + 2
    } else {
        let fallback = 1
        fallback
    }
    return answer
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_else_value() {
    let project = TempProject::new("cli-build-run-terminal-if");
    let source = project.write_source(
        "terminal_if_exit.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 42
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_equality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_inequality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_greater_value() {
    let project = TempProject::new("cli-build-run-terminal-if-greater");
    let source = project.write_source(
        "terminal_if_greater_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value > 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_local_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let enabled = true
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_or_binding_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-or");
    let source = project.write_source(
        "terminal_if_bool_or_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let ready = false
    let fallback = true
    let enabled = ready || fallback
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}
