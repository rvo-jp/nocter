use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_imported_function_call_exit_code() {
    let project = TempProject::new("cli-run-imported-function-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "call.nct",
        r#"use std/math.answer

func main(): i32 {
    return answer()
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
fn run_command_returns_block_scoped_imported_function_call_exit_code() {
    let project = TempProject::new("cli-run-block-scoped-imported-function-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "block_call.nct",
        r#"func main(): i32 {
    use std/math.answer
    return answer()
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
fn run_command_returns_imported_alias_function_call_exit_code() {
    let project = TempProject::new("cli-run-imported-alias-function-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "call_alias.nct",
        r#"use std/math.answer as imported_answer

func main(): i32 {
    return imported_answer()
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
fn run_command_returns_alias_parameter_and_return_exit_code() {
    let project = TempProject::new("cli-run-alias-parameter-return");
    let source = project.write_source(
        "alias_parameter_return.nct",
        r#"type Exit = i32

func main(): i32 {
    return answer(42)
}

func answer(value: Exit): Exit {
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
fn run_command_accepts_alias_entry_return_type() {
    let project = TempProject::new("cli-run-alias-entry-return");
    let source = project.write_source(
        "alias_entry_return.nct",
        r#"type Exit = i32

func main(): Exit {
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
fn run_command_uses_alias_view_signature_for_call_arguments() {
    let project = TempProject::new("cli-run-alias-view-signature-call");
    let source = project.write_source(
        "alias_view_signature_call.nct",
        r#"type Exit = i32
type Text = str

func main(): i32 {
    return length("Nocter")
}

func length(text: &Text): Exit {
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
fn run_command_returns_imported_bool_condition_exit_code() {
    let project = TempProject::new("cli-run-imported-bool-condition");
    project.write_nocter_home_file(
        "std/flags.nct",
        r#"pub func ready(): bool {
    return true
}
"#,
    );
    let source = project.write_source(
        "condition.nct",
        r#"use std/flags.ready

func main(): i32 {
    if ready() {
        return 42
    } else {
        return 1
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
fn run_command_returns_imported_nested_argument_exit_code() {
    let project = TempProject::new("cli-run-imported-nested-argument");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func base(): i32 {
    return 41
}

pub func add_one(value: i32): i32 {
    return value + 1
}
"#,
    );
    let source = project.write_source(
        "nested.nct",
        r#"use std/math.add_one
use std/math.base

func main(): i32 {
    return add_one(base())
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
fn run_command_returns_value_control_with_imported_alias_context_exit_code() {
    let project = TempProject::new("cli-run-imported-alias-value-control-context");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub type Count = i32

pub func zero(): Count {
    return 0
}

pub func choose(value: Count): Count {
    return value
}
"#,
    );
    let source = project.write_source(
        "imported_alias_value_control_context.nct",
        r#"use std/math.{choose, zero}

func main(): i32 {
    var value = zero()
    value = if true { 40 } else { 1 }
    return choose(if value == 40 { value + 2 } else { 1 })
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
fn run_command_discards_imported_alias_scalar_call_exit_code() {
    let project = TempProject::new("cli-run-discard-imported-alias-scalar-call");
    project.write_nocter_home_file(
        "std/metrics.nct",
        r#"pub type Count = i32

pub func record(value: Count): Count {
    return value
}
"#,
    );
    let source = project.write_source(
        "discard_imported_alias_scalar_call.nct",
        r#"use std/metrics.record

func main(): i32 {
    record(1)
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
fn run_command_returns_imported_usize_condition_exit_code() {
    let project = TempProject::new("cli-run-imported-usize-condition");
    project.write_nocter_home_file(
        "std/sizes.nct",
        r#"pub func size(): usize {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_usize_condition.nct",
        r#"use std/sizes.size

func main(): i32 {
    let value: usize = size()
    if value == 42 {
        return 42
    } else {
        return 1
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
fn run_command_returns_alias_i32_conversion_exit_code() {
    let project = TempProject::new("cli-run-alias-i32-conversion");
    let source = project.write_source(
        "alias_i32_conversion.nct",
        r#"type Exit = i32

func main(): i32 {
    return "A"[0] as Exit
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(65),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_imported_interface_default_method() {
    let project = TempProject::new("cli-run-imported-interface-default");
    project.write_nocter_home_file(
        "std/values.nct",
        r#"pub interface Value {
    pub method &self.value(): i32 {
        return 42
    }
}

pub copy struct Unit {
    marker: i32
}

impl Value for Unit

pub func make(): Unit {
    return Unit { marker: 0 }
}
"#,
    );
    let source = project.write_source(
        "imported_interface_default.nct",
        r#"use std/values.make

func main(): i32 {
    let unit = make()
    return unit.value()
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
