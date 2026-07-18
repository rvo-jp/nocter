use super::check_text;

#[test]
fn accepts_switch_over_enum() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
    match error {
        AppError.missing_path {
            return "missing"
        }

        AppError.open_failed(path) {
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
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
    match error {
        AppError.missing_path {
            return "missing"
        }

        AppError.open_failed(path) {
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
fn accepts_exhaustive_switch_without_else_as_terminal_statement() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }

        AppError.open_failed {
            return 2
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
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
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
fn accepts_pattern_conditional_over_enum() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
    return error ?{
        AppError.missing_path : "missing"
        AppError.open_failed(path) : path
        : "unknown"
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_pattern_conditional_contextual_integer_fallback() {
    let diagnostics = check_text(
        r#"enum Status {
    found(code: u8)
    missing
}

func main(): i32 {
    return widen(select(Status.found(7)))
}

func select(status: Status): u8 {
    return status ?{
        Status.missing : 0
        Status.found(code) : code
        : 1
    }
}

func widen(value: u8): i32 {
    return value as i32
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_pattern_conditional_non_enum_target() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 1 ?{
        AppError.missing_path : 1
        : 0
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0361");
}

#[test]
fn diagnoses_pattern_conditional_arm_type_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    return error ?{
        AppError.missing_path : "missing"
        : 0
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0366");
}

#[test]
fn diagnoses_if_is_non_enum_target() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
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

func main(): i32 {
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

func main(): i32 {
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
    open_failed(path: &str)
}

func main(): i32 {
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
fn diagnoses_duplicate_pattern_conditional_arm_variant() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    return error ?{
        AppError.missing_path : 1
        AppError.missing_path : 2
        : 0
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0399");
}

#[test]
fn diagnoses_switch_else_with_non_terminal_arm() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
    match error {
        AppError.missing_path {
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

func main(): i32 {
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
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func make(path: &str): AppError {
    return AppError.open_failed(path)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_enum_variant_construction_as_fallible_failure() {
    let diagnostics = check_text(
        r#"enum AppError {
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func run(path: &str): void! {
    return AppError.open_failed(path)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0311");
}

#[test]
fn diagnoses_unknown_enum_variant_construction() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
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
    open_failed(path: &str)
}

func main(): i32 {
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

func main(): i32 {
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
    open_failed(path: &str)
}

func main(): i32 {
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

func main(): i32 {
    match 1 {
        AppError.missing_path {
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

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        OtherError.missing_path {
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

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.open_failed {
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
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.open_failed {
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
fn diagnoses_duplicate_switch_arm_variant() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }

        AppError.missing_path {
            return 2
        }
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0398");
}

#[test]
fn diagnoses_switch_as_non_terminal_statement() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 1
        }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}
