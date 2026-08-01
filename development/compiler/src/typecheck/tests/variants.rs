use super::check_text;

#[test]
fn diagnoses_non_copy_if_is_payload_binding_from_unmoved_local() {
    let diagnostics = check_text(
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    if result is Result.ok(detail) {
        return detail.code
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0438");
    assert!(
        diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|help| help.contains("if move result"))
    );
}

#[test]
fn accepts_non_copy_if_is_payload_binding_from_moved_local() {
    let diagnostics = check_text(
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    if move result is Result.ok(detail) {
        return detail.code
    }
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_copy_match_payload_binding_from_unmoved_local() {
    let diagnostics = check_text(
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

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0438");
    assert!(
        diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|help| help.contains("match move result"))
    );
}

#[test]
fn diagnoses_non_copy_match_payload_binding_from_member_without_invalid_move_help() {
    let diagnostics = check_text(
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

struct Holder {
    result: Result
}

func main(): i32 {
    let holder = Holder { result: Result.failed }
    return match holder.result {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0438");
    let help = diagnostics[0].help.as_deref().unwrap_or_default();
    assert!(help.contains("member pattern target"), "{diagnostics:?}");
    assert!(!help.contains("move holder.result"), "{diagnostics:?}");
}

#[test]
fn accepts_non_copy_match_payload_binding_from_owned_temporary() {
    let diagnostics = check_text(
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    return match Result.ok(Detail { code: 42 }) {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

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
fn substitutes_generic_enum_switch_payload_binding_type() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func value(option: Maybe<i32>): i32 {
    match option {
        Maybe.some(inner) {
            return inner
        }

        Maybe.empty {
            return 0
        }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_generic_enum_switch_payload_binding_type_mismatch() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func value(option: Maybe<i32>): &str {
    match option {
        Maybe.some(inner) {
            return inner
        }

        Maybe.empty {
            return "none"
        }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn accepts_switch_wildcard_as_terminal_statement() {
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

        _ {
            return "unknown"
        }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_exhaustive_switch_without_wildcard_fallback_as_terminal_statement() {
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
fn accepts_payload_discard_patterns() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return describe(AppError.missing_path)
}

func describe(error: AppError): i32 {
    if error is AppError.open_failed(_) {
        return 2
    }

    match error {
        AppError.open_failed(_) {
            return 1
        }

        _ {
            return 0
        }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn substitutes_generic_enum_if_is_payload_binding_type() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func describe(value: Maybe<&str>): &str {
    if value is Maybe.some(text) {
        return text
    }

    return "none"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_match_expression_over_enum() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    return 0
}

func describe(error: AppError): &str {
    return match error {
        AppError.missing_path { "missing" }
        AppError.open_failed(path) { path }
        _ { "unknown" }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn substitutes_generic_enum_match_expression_payload_binding_type() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func value(option: Maybe<i32>): i32 {
    return match option {
        Maybe.some(inner) { inner }
        Maybe.empty { 0 }
        _ { 0 }
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_generic_enum_match_expression_payload_type_mismatch() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func value(option: Maybe<i32>): &str {
    return match option {
        Maybe.some(inner) { inner }
        Maybe.empty { "none" }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0366");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn accepts_match_expression_contextual_integer_fallback() {
    let diagnostics = check_text(
        r#"enum Status {
    found(code: u8)
    missing
}

func main(): i32 {
    return widen(select(Status.found(7)))
}

func select(status: Status): u8 {
    return match status {
        Status.missing { 0 }
        Status.found(code) { code }
        _ { 1 }
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
fn diagnoses_match_expression_non_enum_target() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return match 1 {
        AppError.missing_path { 1 }
        _ { 0 }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0361");
}

#[test]
fn diagnoses_match_expression_arm_type_mismatch() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    return match error {
        AppError.missing_path { "missing" }
        _ { 0 }
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
fn diagnoses_duplicate_match_expression_arm_variant() {
    let diagnostics = check_text(
        r#"enum AppError {
    missing_path
    open_failed
}

func main(): i32 {
    return 0
}

func code(error: AppError): i32 {
    return match error {
        AppError.missing_path { 1 }
        AppError.missing_path { 2 }
        _ { 0 }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0398");
}

#[test]
fn diagnoses_switch_wildcard_with_non_terminal_arm() {
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

        _ {
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
fn infers_generic_enum_variant_type_from_payload() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func make(): Maybe<i32> {
    return Maybe.some(1)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_generic_payloadless_enum_variant() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func make(): Maybe<i32> {
    return Maybe.empty
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_generic_enum_variant_payload_type_mismatch_from_context() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func make(): Maybe<i32> {
    return Maybe.some("bad")
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("Maybe<&str>"));
    assert!(diagnostics[0].message.contains("Maybe<i32>"));
}

#[test]
fn diagnoses_repeated_generic_enum_variant_payload_mismatch() {
    let diagnostics = check_text(
        r#"enum Pair<T> {
    same(left: T, right: T)
}

func main(): i32 {
    let pair = Pair.same(1, "bad")
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0368");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn accepts_generic_enum_variant_construction_in_generic_function() {
    let diagnostics = check_text(
        r#"enum Maybe<T> {
    some(value: T)
    empty
}

func main(): i32 {
    return 0
}

func make<T>(value: T): Maybe<T> {
    return Maybe.some(value)
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
