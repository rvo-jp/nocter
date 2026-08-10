use super::*;

#[test]
fn build_command_lowers_terminal_if_branch_drop() {
    let project = TempProject::new("cli-build-terminal-if-branch-drop");
    let source = project.write_source(
        "terminal_if_branch_drop.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        drop file
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
fn build_command_lowers_nonterminal_if_branch_scope_drop() {
    let project = TempProject::new("cli-build-nonterminal-if-branch-scope-drop");
    let source = project.write_source(
        "nonterminal_if_branch_scope_drop.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var file = File { fd: 1 }
    } else {
        var file = File { fd: 2 }
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
fn build_command_lowers_nonterminal_if_branch_outer_move_binding_before_return() {
    let project =
        TempProject::new("cli-build-nonterminal-if-branch-outer-move-binding-before-return");
    let source = project.write_source(
        "nonterminal_if_branch_outer_move_binding_before_return.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        var moved = move file
        return moved.fd
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
fn build_command_lowers_nonterminal_if_branch_outer_move_assignment_before_return() {
    let project =
        TempProject::new("cli-build-nonterminal-if-branch-outer-move-assignment-before-return");
    let source = project.write_source(
        "nonterminal_if_branch_outer_move_assignment_before_return.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        return target.fd
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
fn build_command_lowers_nonterminal_while_body_scope_drop() {
    let project = TempProject::new("cli-build-nonterminal-while-body-scope-drop");
    let source = project.write_source(
        "nonterminal_while_body_scope_drop.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
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
fn build_command_lowers_nonterminal_while_body_explicit_drop() {
    let project = TempProject::new("cli-build-nonterminal-while-body-explicit-drop");
    let source = project.write_source(
        "nonterminal_while_body_explicit_drop.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        drop file
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
fn build_command_lowers_nonterminal_while_body_outer_explicit_drop_before_return() {
    let project =
        TempProject::new("cli-build-nonterminal-while-body-outer-explicit-drop-before-return");
    let source = project.write_source(
        "nonterminal_while_body_outer_explicit_drop_before_return.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        drop file
        return 1
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
fn build_command_lowers_nonterminal_while_break_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-while-break-cleanup");
    let source = project.write_source(
        "nonterminal_while_break_cleanup.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        break
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
fn build_command_lowers_nonterminal_while_continue_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-while-continue-cleanup");
    let source = project.write_source(
        "nonterminal_while_continue_cleanup.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        continue
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
fn build_command_lowers_nonterminal_loop_break_cleanup() {
    let project = TempProject::new("cli-build-nonterminal-loop-break-cleanup");
    let source = project.write_source(
        "nonterminal_loop_break_cleanup.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File { fd: 1 }
        break
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
fn build_command_lowers_terminal_loop_body_return_cleanup() {
    let project = TempProject::new("cli-build-terminal-loop-body-return-cleanup");
    let source = project.write_source(
        "terminal_loop_body_return_cleanup.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File { fd: 1 }
        return 7
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
fn build_command_lowers_imported_readwrite_borrow_alias_argument() {
    let project = TempProject::new("cli-build-imported-readwrite-borrow-alias-argument");
    project.write_source(
        "borrow_api/index.nct",
        r#"pub type IntMut = &+i32

pub func choose(value: IntMut, fallback: i32): i32 {
    return fallback
}
"#,
    );
    let source = project.write_source(
        "imported_readwrite_borrow_alias_argument.nct",
        r#"use ./borrow_api.choose

func main(): i32 {
    var value = 1
    return choose(&+value, 42)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_readonly_temporary_scalar_borrow_argument() {
    let project = TempProject::new("cli-build-readonly-temporary-scalar-borrow-argument");
    let source = project.write_source(
        "readonly_temporary_scalar_borrow_argument.nct",
        r#"func main(): i32 {
    return choose(&answer(), 0)
}

func answer(): i32 {
    return 1
}

func choose(value: &i32, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_non_binding_root_borrow_argument() {
    let project = TempProject::new("cli-build-non-binding-root-borrow-argument");
    let source = project.write_source(
        "non_binding_root_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    return choose(&make().value, 0)
}

func make(): Pair {
    return Pair { value: 1 }
}

func choose(value: &i32, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_explicit_move_in_terminal_if_condition() {
    let project = TempProject::new("cli-build-move-in-terminal-if-condition");
    let source = project.write_source(
        "move_in_terminal_if_condition.nct",
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    if consume(move file) {
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
fn build_command_lowers_concrete_generic_scope_end_drop() {
    let project = TempProject::new("cli-build-concrete-generic-scope-end-drop");
    let source = project.write_source(
        "concrete_generic_scope_end_drop.nct",
        r#"struct Box<T> {
    value: T
}

instance Box<T> {
    drop &+self {
        return
    }
}

func main(): i32 {
    var box = Box<i32> { value: 42 }
    return box.value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_temporary_method_borrow_receiver() {
    let project = TempProject::new("cli-build-temporary-method-borrow-receiver");
    let source = project.write_source(
        "temporary_method_borrow_receiver.nct",
        r#"copy struct File {
    fd: i32
}

instance File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    return make_file().value()
}

func make_file(): File {
    return File { fd: 42 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}
