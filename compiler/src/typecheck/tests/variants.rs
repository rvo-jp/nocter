use super::*;

#[test]
fn accepts_switch_over_enum() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            return "missing"
        }

        is AppError.open_failed(path) {
            return path
        }
    }

    return "unknown"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_switch_else_as_terminal_statement() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            return "missing"
        }

        is AppError.open_failed(path) {
            return path
        }

        else {
            return "unknown"
        }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_is_over_enum() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    if error is AppError.open_failed(path) {
        return path
    } else if error is AppError.missing_path {
        return "missing"
    } else {
        return "unknown"
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_if_is_non_enum_target() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    if 1 is AppError.missing_path {
        return 1
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0361");
}

#[test]
fn diagnoses_if_is_enum_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

enum OtherError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is OtherError.missing_path {
        return 1
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0363");
}

#[test]
fn diagnoses_if_is_unknown_variant() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed {
        return 1
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0364");
}

#[test]
fn diagnoses_if_is_payload_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed {
        return 1
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0365");
}

#[test]
fn diagnoses_switch_else_with_non_terminal_arm() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            let message = "missing"
        }

        else {
            return "unknown"
        }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_payloadless_enum_variant_construction() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.missing_path
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_payload_enum_variant_construction() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(path: str): AppError {
    return AppError.open_failed(path)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_enum_variant_construction_in_fail() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func run(path: str): void! {
    fail AppError.open_failed(path)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
}

#[test]
fn diagnoses_unknown_enum_variant_construction() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0366");
}

#[test]
fn diagnoses_enum_variant_payload_count_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0367");
}

#[test]
fn diagnoses_payloadless_enum_variant_call() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.missing_path()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0367");
}

#[test]
fn diagnoses_enum_variant_payload_type_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed(1)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0368");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_switch_non_enum_target() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    switch 1 {
        is AppError.missing_path {
            return 1
        }
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0361");
}

#[test]
fn diagnoses_switch_arm_enum_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

enum OtherError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is OtherError.missing_path {
            return 1
        }
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0363");
}

#[test]
fn diagnoses_switch_unknown_variant() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.open_failed {
            return 1
        }
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0364");
}

#[test]
fn diagnoses_switch_payload_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.open_failed {
            return 1
        }
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0365");
}

#[test]
fn diagnoses_switch_as_non_terminal_statement() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}
