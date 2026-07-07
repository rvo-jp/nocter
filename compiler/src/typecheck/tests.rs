use super::*;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::source::SourceMap;

fn check_text(text: &str) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let resolved = resolve(&sources, &ast);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn accepts_program_i32() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_program_i32_fallible() {
    let diagnostics = check_text(
        r#"program(): i32! {
    return run()?
}

func run(): i32! {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_program_void() {
    let diagnostics = check_text(
        r#"program(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_main_without_program() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0301");
}

#[test]
fn diagnoses_invalid_program_return_type() {
    let diagnostics = check_text(
        r#"program(): u64 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0303");
}

#[test]
fn diagnoses_duplicate_program() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

program(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0302");
}

#[test]
fn diagnoses_string_return_from_i32_program() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_bare_return_from_i32_program() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0310");
}

#[test]
fn diagnoses_value_return_from_void_program() {
    let diagnostics = check_text(
        r#"program(): void {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0311");
}

#[test]
fn diagnoses_missing_return_from_i32_program() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_str_function_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func title(): str {
    return "hello"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_struct_field_access() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    label: str
}

program(): i32 {
    return 0
}

func x(point: Point): i32 {
    return point.x
}

func label(point: Point): str {
    return point.label
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn uses_struct_field_type_for_return_checking() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

program(): i32 {
    return 0
}

func x(point: Point): str {
    return point.x
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_unknown_struct_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

program(): i32 {
    return 0
}

func y(point: Point): i32 {
    return point.y
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("Point"));
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_field_access_on_non_struct_value() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func invalid(value: i32): i32 {
    return value.x
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0371");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_struct_literal_expression() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    label: str
}

program(): i32 {
    let point = Point{
        label: "home",
        x: 1,
    }
    return point.x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_integer_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Byte {
    value: u8
}

program(): i32 {
    let byte = Byte{
        value: 255,
    }
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_struct_literal_target() {
    let diagnostics = check_text(
        r#"type Number = i32

program(): i32 {
    let value = Number{
        value: 1,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0372");
}

#[test]
fn diagnoses_unknown_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

program(): i32 {
    let point = Point{
        x: 1,
        y: 2,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0373");
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_duplicate_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

program(): i32 {
    let point = Point{
        x: 1,
        x: 2,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0374");
    assert!(diagnostics[0].message.contains("x"));
}

#[test]
fn diagnoses_missing_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    y: i32
}

program(): i32 {
    let point = Point{
        x: 1,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0375");
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_struct_literal_field_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

program(): i32 {
    let point = Point{
        x: "bad",
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0376");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn displays_fixed_array_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return "nope"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("[u8; 4]"));
}

#[test]
fn accepts_contextual_fixed_array_literal_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C, 0x46]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_fixed_array_literal_length_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C]
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("[i32; 3]"));
    assert!(diagnostics[0].message.contains("[u8; 4]"));
}

#[test]
fn accepts_contextual_fixed_array_literal_binding() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_array_literal_element_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let items = [1, "two"]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0343");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_binding_annotation_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let byte: u8 = 300
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("u8"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_fixed_array_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(): u8 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return header[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_view_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(bytes: [u8]): u8 {
    return bytes[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_str_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(): u8 {
    return "hello"[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_bool_function_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func enabled(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_else_return_as_terminal_statement() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_condition_from_bool_binding() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let enabled = true
    if enabled {
        return 0
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_condition_from_comparison() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let count = 1
    if count > 0 {
        return 0
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_equality_comparison_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
fn diagnoses_non_bool_if_condition() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if 1 {
        return 0
    }
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0346");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_if_without_else_as_non_terminal() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if true {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn diagnoses_equality_operand_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
    return 0
}

func same(): bool {
    return "a" == "b"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_index_on_non_indexable_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let number = 1
    let byte = number[0]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0344");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_non_integer_index_value() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    let byte = header["0"]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0345");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_optional_none_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func lookup(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_annotated_optional_binding_from_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value: i32? = maybe_answer()
    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_let_else_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = maybe_answer() else {
        return 1
    }

    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_let_else_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = 1 else {
        return 1
    }

    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0340");
}

#[test]
fn diagnoses_optional_let_else_fallthrough() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = maybe_answer() else {
        log_missing()
    }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func log_missing(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0341");
}

#[test]
fn uses_optional_let_else_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = maybe_title() else {
        return 1
    }

    return value
}

func maybe_title(): str? {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_optional_if_let_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if let value = maybe_answer() {
        return value
    } else {
        return 0
    }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_if_var_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if var value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_if_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if let value = 1 {
        return value
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0356");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn uses_optional_if_let_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_else_if_let_terminal_chain() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if false {
        return 0
    } else if let value = maybe_answer() {
        return value
    } else if var fallback = maybe_fallback() {
        return fallback
    } else {
        return 3
    }
}

func maybe_answer(): i32? {
    return none
}

func maybe_fallback(): i32? {
    return 2
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_else_if_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
    if false {
        return 0
    } else if let value = 1 {
        return value
    } else {
        return 2
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0356");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_while_bool_condition() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while ready() {
        tick()
    }

    return 0
}

func ready(): bool {
    return false
}

func tick(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_while_let_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while let value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_while_var_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while var value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_bool_while_condition() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while 1 {
        return 0
    }

    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0357");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_optional_while_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while let value = 1 {
        return value
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0358");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn uses_optional_while_let_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    while let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_break_and_continue_inside_loops() {
    let diagnostics = check_text(
        r#"program(): void {
    while ready() {
        break
    }

    while let value = maybe_answer() {
        continue
    }
}

func ready(): bool {
    return true
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_inside_loop_expression_catch_block() {
    let diagnostics = check_text(
        r#"program(): void {
    while ready() {
        let value = fallible() catch error {
            break
        }
    }
}

func ready(): bool {
    return true
}

func fallible(): i32! {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_and_continue_inside_loop_statement() {
    let diagnostics = check_text(
        r#"program(): void {
    loop {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_range_for_integer_bounds() {
    let diagnostics = check_text(
        r#"program(): i32 {
    for i in 0..<4 {
        return i
    }

    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_range_for_contextual_integer_literal_bound() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(limit: u64): u64 {
    for i in 0..<limit {
        return i
    }

    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_and_continue_inside_range_for() {
    let diagnostics = check_text(
        r#"program(): void {
    for i in 0..<4 {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_range_for_non_integer_bound() {
    let diagnostics = check_text(
        r#"program(): i32 {
    for i in "a"..<4 {
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0360");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_range_for_bound_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let start: u16 = 0
    let end: u8 = 4

    for i in start..<end {
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0360");
    assert!(diagnostics[0].message.contains("u16"));
    assert!(diagnostics[0].message.contains("u8"));
}

#[test]
fn diagnoses_range_for_as_non_terminal_statement() {
    let diagnostics = check_text(
        r#"program(): i32 {
    for i in 0..<1 {
        return i
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_loop_with_return_as_terminal_statement() {
    let diagnostics = check_text(
        r#"program(): i32 {
    loop {
        return 0
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_terminal_loop_with_break() {
    let diagnostics = check_text(
        r#"program(): i32 {
    loop {
        break
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn diagnoses_break_outside_loop() {
    let diagnostics = check_text(
        r#"program(): void {
    break
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0359");
    assert!(diagnostics[0].message.contains("break"));
}

#[test]
fn diagnoses_continue_outside_loop() {
    let diagnostics = check_text(
        r#"program(): void {
    continue
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0359");
    assert!(diagnostics[0].message.contains("continue"));
}

#[test]
fn checks_success_type_of_fallible_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(): void! {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_fail_in_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    fail error
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_code_and_message_fields() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func error_code(error: error): str {
    return error.code
}

func error_message(error: error): str {
    return error.message
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_fields_inside_catch_block() {
    let diagnostics = check_text(
        r#"program(): i32! {
    run() catch error {
        report(error.code)
        report(error.message)
        return 1
    }

    return 0
}

func run(): i32! {
    return 0
}

func report(text: str): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unknown_error_field() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func error_code(error: error): str {
    return error.raw_code
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0369");
    assert!(diagnostics[0].message.contains("raw_code"));
}

#[test]
fn diagnoses_non_error_fail_value() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(): i32! {
    fail 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("error"));
}

#[test]
fn diagnoses_fail_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: u64): i32 {
    fail error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0333");
}

#[test]
fn diagnoses_fail_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: str): i32! {
    fail error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("error"));
}

#[test]
fn accepts_fail_as_terminal_branch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    if true {
        fail error
    } else {
        return 0
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

#[test]
fn uses_same_file_function_call_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return title()
}

func title(): str {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_same_file_function_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer()
}

func answer(value: i32): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0320");
}

#[test]
fn diagnoses_same_file_function_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return length("hello")
}

func length(value: i32): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn accepts_associated_function_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    pub func origin(): Self {
        return Self{ x: 0 }
    }
}

program(): i32 {
    return Point.origin().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_associated_function_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
    assert!(diagnostics[0].message.contains("Parser.parse"));
}

#[test]
fn diagnoses_associated_function_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    return Parser.parse("bad")
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn diagnoses_associated_function_body_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(): i32 {
        return "bad"
    }
}

program(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(
        diagnostics[0]
            .message
            .contains("associated function `Parser.parse`")
    );
}

#[test]
fn diagnoses_associated_function_body_call_argument_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(): i32 {
        return needs_value()
    }
}

func needs_value(value: i32): i32 {
    return value
}

program(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
}

#[test]
fn accepts_method_body_receiver_self_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return point.x
    }
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_method_call_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return point.x
    }
}

program(): i32 {
    let point = Point{ x: 1 }
    return point.x_value()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_method_call_self_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).same(): Self {
        return Self{ x: point.x }
    }
}

program(): i32 {
    let point = Point{ x: 1 }
    return point.same().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_method_call_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    method (parser: Self).parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    let parser = Parser{ value: 0 }
    return parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
    assert!(diagnostics[0].message.contains("method `Parser.parse`"));
}

#[test]
fn diagnoses_method_call_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    method (parser: Self).parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    let parser = Parser{ value: 0 }
    return parser.parse("bad")
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn accepts_readwrite_method_call_on_var_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    var file = File{ fd: 1 }
    return file.write()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_readwrite_method_call_on_let_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    return file.write()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0378");
}

#[test]
fn diagnoses_readwrite_method_call_on_temporary() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    return File{ fd: 1 }.write()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0378");
}

#[test]
fn accepts_readonly_method_call_on_let_binding_and_temporary() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &Self).fd_value(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    if file.fd_value() == 0 {
        return File{ fd: 2 }.fd_value()
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unsupported_method_receiver_type() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: i32).bad(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    return file.bad()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0377");
}

#[test]
fn diagnoses_method_body_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return "bad"
    }
}

program(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("method `Point.x_value`"));
}

#[test]
fn unwraps_catch_expression_success_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_propagation_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer()?
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0331");
}

#[test]
fn diagnoses_catch_on_non_fallible_expression() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0330");
}
