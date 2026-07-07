use super::support::resolve_text;

#[test]
fn diagnoses_builtin_error_type_name_reuse() {
    let output = resolve_text(
        r#"type error = i32

program(): i32 {
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
        r#"program(): i32 {
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
fn diagnoses_duplicate_inherent_associated_function_names_in_same_impl() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

impl File {
    func open(): i32 {
        return 0
    }

    func open(path: str): i32 {
        return 1
    }
}

program(): i32 {
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

program(): i32 {
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

impl File {
    func open(): i32 {
        return 0
    }

    method (file: Self).open(): i32 {
        return file.fd
    }
}

program(): i32 {
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
        r#"program(): i32 {
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
