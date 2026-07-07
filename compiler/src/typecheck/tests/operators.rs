use super::check_text;

#[test]
fn accepts_equality_comparison_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(): bool {
    return true == false
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_integer_literal_comparison() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func is_zero(byte: u8): bool {
    return byte == 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_logical_expression_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func enabled(left: bool, right: bool, count: i32): bool {
    return left && count > 0 || right
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_logical_not_expression_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func disabled(enabled: bool): bool {
    return !enabled
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_numeric_negate_expression_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func negative(value: i32): i32 {
    return -value
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_negative_integer_literal_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value: i64 = -1
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_arithmetic_expression_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func calc(left: i32, right: i32): i32 {
    return left + right * 2 - 4 / 2 % 2
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_integer_literal_arithmetic() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func add_one(byte: u8): u8 {
    return byte + 1
}

func add_one_reversed(byte: u8): u8 {
    return 1 + byte
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_lossless_integer_type_conversion() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let literal = 10 as u8
    return 0
}

func widen_small(value: u8): u16 {
    return value as u16
}

func widen_large(value: u32): u64 {
    return value as u64
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_shift_expression_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func shift_left(value: u64, count: u8): u64 {
    return value << count
}

func shift_right(value: i32): i32 {
    return value >> 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_condition_from_logical_expression() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let count = 1
    let ready = true
    if count > 0 && ready {
        return 0
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_equality_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let same = 1 == "1"
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0347");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_ordered_comparison_on_non_integer_operands() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let less = true < false
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0348");
    assert!(diagnostics[0].message.contains("bool"));
}

#[test]
fn diagnoses_ordered_comparison_integer_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func less(left: u8, right: u16): bool {
    return left < right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0348");
    assert!(diagnostics[0].message.contains("u8"));
    assert!(diagnostics[0].message.contains("u16"));
}

#[test]
fn diagnoses_arithmetic_integer_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func calc(left: u8, right: u16): void {
    let invalid = left + right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0352");
    assert!(diagnostics[0].message.contains("u8"));
    assert!(diagnostics[0].message.contains("u16"));
}

#[test]
fn diagnoses_arithmetic_on_non_integer_operands() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = true + false
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0352");
    assert!(diagnostics[0].message.contains("bool"));
}

#[test]
fn diagnoses_narrowing_integer_type_conversion() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func narrow(value: u64): void {
    let invalid = value as u8
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0355");
    assert!(diagnostics[0].message.contains("u64"));
    assert!(diagnostics[0].message.contains("u8"));
}

#[test]
fn diagnoses_signed_to_unsigned_type_conversion() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func convert(value: i32): void {
    let invalid = value as u64
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0355");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("u64"));
}

#[test]
fn diagnoses_non_integer_type_conversion() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = true as i32
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0355");
    assert!(diagnostics[0].message.contains("bool"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_shift_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = 1 << false
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0353");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("bool"));
}

#[test]
fn diagnoses_negative_shift_count() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = 1 << -1
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0354");
}

#[test]
fn diagnoses_logical_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = true && 1
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0349");
    assert!(diagnostics[0].message.contains("bool"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_logical_not_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let invalid = !1
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0350");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_numeric_negate_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func negative(value: u8): u8 {
    return -value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0351");
    assert!(diagnostics[0].message.contains("u8"));
}

#[test]
fn diagnoses_negative_integer_literal_unsigned_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value: u8 = -1
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("u8"));
}

#[test]
fn accepts_str_equality_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(): bool {
    return "a" == "b"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
