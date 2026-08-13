use super::check_text;

#[test]
fn accepts_instance_owned_equality_and_generic_requirement() {
    let diagnostics = check_text(
        r#"struct Text { value: i32 }

instance Text {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

func equal<T>(left: &T, right: &T): bool where (&T == &T): bool {
    return left == right
}

func main(): i32 {
    let left = Text { value: 1 }
    let right = Text { value: 1 }
    if equal(&left, &right) { return 0 }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_instance_owned_strict_order_and_all_derived_spellings() {
    let diagnostics = check_text(
        r#"struct Rank { value: i32 }

instance Rank {
    operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}

func less<T>(left: &T, right: &T): bool where (&T < &T): bool {
    return left < right
}

func main(): i32 {
    let low = Rank { value: 1 }
    let high = Rank { value: 2 }
    if !less(&low, &high) { return 1 }
    if low >= high { return 2 }
    if high <= low { return 3 }
    if low > high { return 4 }
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_malformed_and_unsatisfied_strict_order_contracts() {
    let malformed = check_text(
        r#"struct Rank {}
instance Rank {
    operator (&self < other: &i32): i32 { return 0 }
}
func main(): i32 { return 0 }
"#,
    );
    assert!(
        malformed
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0470")
            .count()
            >= 2,
        "{malformed:?}"
    );

    let unsatisfied = check_text(
        r#"struct Rank { value: i32 }
func less<T>(left: &T, right: &T): bool where (&T < &T): bool {
    return left < right
}
func main(): i32 {
    let left = Rank { value: 1 }
    let right = Rank { value: 2 }
    if less(&left, &right) { return 1 }
    return 0
}
"#,
    );
    assert!(
        unsatisfied.iter().any(|diagnostic| diagnostic.code == "E0473"
            && diagnostic.message.contains("ordering")),
        "{unsatisfied:?}"
    );
}

#[test]
fn diagnoses_malformed_equality_declaration_contracts() {
    let wrong_operand = check_text(
        r#"struct Text {}
instance Text {
    operator (&self == other: &i32): bool { return true }
}

func main(): i32 { return 0 }
"#,
    );
    assert!(
        wrong_operand
            .iter()
            .any(|diagnostic| diagnostic.code == "E0470"
                && diagnostic.message.contains("right operand")),
        "{wrong_operand:?}"
    );

    let wrong_result = check_text(
        r#"struct Text {}
instance Text {
    operator (&self == other: &Self): i32 { return 1 }
}
func main(): i32 { return 0 }
"#,
    );
    assert!(
        wrong_result
            .iter()
            .any(|diagnostic| diagnostic.code == "E0470"
                && diagnostic.message.contains("return type")),
        "{wrong_result:?}"
    );
}

#[test]
fn diagnoses_duplicate_equality_declarations_on_one_instance_surface() {
    let diagnostics = check_text(
        r#"struct Text { value: i32 }
instance Text {
    operator (&self == left: &Self): bool { return true }
    operator (&self == right: &Self): bool { return false }
}
func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("duplicate")
                || diagnostic.message.contains("overlap")
                || diagnostic.message.contains("already")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_invalid_duplicate_and_unsatisfied_equality_requirements() {
    let invalid = check_text(
        r#"func equal<T, U>(left: &T, right: &U): bool where (&T == &U): bool {
    return false
}
func main(): i32 { return 0 }
"#,
    );
    assert!(
        invalid.iter().any(|diagnostic| diagnostic.code == "E0471"),
        "{invalid:?}"
    );

    let duplicate = check_text(
        r#"func equal<T>(left: &T, right: &T): bool where (&T == &T): bool, (&T == &T): bool {
    return left == right
}
func main(): i32 { return 0 }
"#,
    );
    assert!(
        duplicate
            .iter()
            .any(|diagnostic| diagnostic.code == "E0472"),
        "{duplicate:?}"
    );

    let unsatisfied = check_text(
        r#"struct Text { value: i32 }
func equal<T>(left: &T, right: &T): bool where (&T == &T): bool {
    return left == right
}
func main(): i32 {
    let left = Text { value: 1 }
    let right = Text { value: 1 }
    if equal(&left, &right) { return 1 }
    return 0
}
"#,
    );
    assert!(
        unsatisfied
            .iter()
            .any(|diagnostic| diagnostic.code == "E0473"),
        "{unsatisfied:?}"
    );
}

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
fn accepts_payloadless_enum_equality_return() {
    let diagnostics = check_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    return 0
}

func same(): bool {
    return Choice.yes != Choice.no
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_payload_enum_equality_return() {
    let diagnostics = check_text(
        r#"enum Choice {
    yes
    number(value: i32)
}

func main(): i32 {
    return 0
}

func same(): bool {
    return Choice.yes == Choice.yes
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0347");
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
    let converted = 10 as u8
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

func shift_left(value: u64, count: u64): u64 {
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
fn diagnoses_equality_ambiguity_across_readonly_coercions() {
    let diagnostics = check_text(
        r#"struct First { value: i32 }
struct Second { value: i32 }

instance First {
    operator (&self == other: &Self): bool { return self.value == other.value }
}

instance Second {
    operator (&self == other: &Self): bool { return self.value == other.value }
}

struct Owner {
    first: First,
    second: Second,
}

instance Owner {
    coerce &self as &First { return &self.first }
    coerce &self as &Second { return &self.second }
}

func same(left: &Owner, right: &Owner): bool {
    return left == right
}

func main(): i32 { return 0 }
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0474");
    assert_eq!(diagnostics[0].notes.len(), 2, "{diagnostics:?}");
}

#[test]
fn diagnoses_ordering_ambiguity_across_readonly_coercions() {
    let diagnostics = check_text(
        r#"struct First { value: i32 }
struct Second { value: i32 }

instance First {
    operator (&self < other: &Self): bool { return self.value < other.value }
}

instance Second {
    operator (&self < other: &Self): bool { return self.value < other.value }
}

struct Owner { first: First, second: Second }

instance Owner {
    coerce &self as &First { return &self.first }
    coerce &self as &Second { return &self.second }
}

func less(left: &Owner, right: &Owner): bool { return left < right }
func main(): i32 { return 0 }
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0474");
    assert!(diagnostics[0].message.contains("ordering"));
    assert_eq!(diagnostics[0].notes.len(), 2, "{diagnostics:?}");
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
fn diagnoses_shift_count_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value: i32 = 1
    let count: u8 = 1
    let invalid = value << count
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0353");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("u8"));
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
fn diagnoses_move_non_binding_operand() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return move (1 + 2)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("binding"));
}

#[test]
fn diagnoses_move_of_copy_field() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    return move text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0394");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_move_call_result_operand() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return move value()
}

func value(): i32 {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("binding"));
}

#[test]
fn diagnoses_move_index_operand() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func first(): u8 {
    let values: [u8; 2] = [1, 2]
    return move values[0]
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("binding"));
}

#[test]
fn diagnoses_move_of_copy_scalar_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 42
    return move value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0394");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_move_of_borrow_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 42
    let borrowed = &value
    let moved = move borrowed
    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0394");
    assert!(diagnostics[0].message.contains("&i32"));
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
