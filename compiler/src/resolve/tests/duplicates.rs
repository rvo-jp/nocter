use super::support::resolve_text;

#[test]
fn diagnoses_builtin_error_type_name_reuse() {
    let output = resolve_text(
        r#"type error = i32

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0401");
}

#[test]
fn diagnoses_duplicate_function_names() {
    let output = resolve_text(
        r#"func main(): i32 {
    return 0
}

func answer(): i32 {
    return 1
}

func answer(): i32 {
    return 2
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].code, "E0400");
}

#[test]
fn diagnoses_associated_function_owner_that_is_not_a_type() {
    let output = resolve_text(
        r#"func Parser(): i32 {
    return 0
}

func Parser.new(): i32 {
    return 1
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0414");
}

#[test]
fn diagnoses_unknown_associated_function_owner() {
    let output = resolve_text(
        r#"func Parser.new(): i32 {
    return 1
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0414");
}

#[test]
fn diagnoses_duplicate_inherent_associated_function_names() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

func File.open(): i32 {
    return 0
}

func File.open(path: &str): i32 {
    return 1
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_duplicate_inherent_method_names_across_impls() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: Self).name(): i32 {
        return file.fd
    }
}

impl File {
    method (file: Self).name(value: i32): i32 {
        return value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_duplicate_inherent_function_and_method_name() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

func File.open(): i32 {
    return 0
}

impl File {
    method (file: Self).open(): i32 {
        return file.fd
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_associated_function_name_that_reuses_enum_variant() {
    let output = resolve_text(
        r#"enum AppError {
    missing_path
}

func AppError.missing_path(): i32 {
    return 0
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_method_name_that_reuses_enum_variant() {
    let output = resolve_text(
        r#"enum AppError {
    missing_path
}

impl AppError {
    method (error: Self).missing_path(): i32 {
        return 0
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_duplicate_inherent_drop_members_across_impls() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0413");
}

#[test]
fn diagnoses_local_shadowing_top_level_function() {
    let output = resolve_text(
        r#"func main(): i32 {
    let answer = 0
    return answer
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].code, "E0400");
}
