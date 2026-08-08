use super::*;

#[test]
fn build_command_lowers_imported_usize_call_condition() {
    let project = TempProject::new("cli-build-imported-usize-condition");
    project.write_nocter_home_file(
        "std/sizes/index.nct",
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
fn build_command_lowers_imported_i32_call() {
    let project = TempProject::new("cli-build-imported-call");
    project.write_nocter_home_file(
        "std/math/index.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_call.nct",
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_imported_alias_i32_call() {
    let project = TempProject::new("cli-build-imported-alias-call");
    project.write_nocter_home_file(
        "std/math/index.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_alias_call.nct",
        r#"use std/math.answer as imported_answer

func main(): i32 {
    return imported_answer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_alias_parameter_and_return_abi() {
    let project = TempProject::new("cli-build-alias-parameter-return-abi");
    let source = project.write_source(
        "alias_parameter_return_abi.nct",
        r#"type Exit = i32
type Text = str
type Bytes = [u8]

func main(): i32 {
    return 0
}

func answer(name: &Text, code: Exit): Exit {
    return code
}

func echo(bytes: &+Bytes): &+Bytes {
    return bytes
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
fn build_command_lowers_reachable_alias_view_signature() {
    let project = TempProject::new("cli-build-reachable-alias-view-signature");
    let source = project.write_source(
        "alias_view_signature.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_imported_bool_condition() {
    let project = TempProject::new("cli-build-imported-bool-condition");
    project.write_nocter_home_file(
        "std/flags/index.nct",
        r#"pub func ready(): bool {
    return true
}
"#,
    );
    let source = project.write_source(
        "imported_bool_condition.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_imported_nested_argument() {
    let project = TempProject::new("cli-build-imported-nested-argument");
    project.write_nocter_home_file(
        "std/math/index.nct",
        r#"pub func base(): i32 {
    return 41
}

pub func add_one(value: i32): i32 {
    return value + 1
}
"#,
    );
    let source = project.write_source(
        "imported_nested_argument.nct",
        r#"use std/math.add_one
use std/math.base

func main(): i32 {
    return add_one(base())
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_value_control_with_imported_alias_context() {
    let project = TempProject::new("cli-build-imported-alias-value-control-context");
    project.write_nocter_home_file(
        "std/math/index.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_discards_imported_alias_scalar_call() {
    let project = TempProject::new("cli-build-discard-imported-alias-scalar-call");
    project.write_nocter_home_file(
        "std/metrics/index.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[test]
fn build_command_lowers_loaded_imported_i32_range_for() {
    let project = TempProject::new("cli-build-imported-range-for");
    project.write_nocter_home_file(
        "std/loops/index.nct",
        r#"pub func helper(): i32 {
    var total = 0
    for value in 0..<4 {
        total = total + value
    }

    return total
}
"#,
    );
    let source = project.write_source(
        "imported_range_for.nct",
        r#"use std/loops.helper

func main(): i32 {
    return helper()
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
