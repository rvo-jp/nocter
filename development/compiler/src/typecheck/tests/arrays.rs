use super::check_text;

#[test]
fn displays_fixed_array_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
        r#"func main(): i32 {
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
        r#"func main(): i32 {
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
        r#"func main(): i32 {
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
        r#"func main(): i32 {
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
fn accepts_fixed_array_index_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): u8 {
    return bytes[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_view_len_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func size(bytes: &[u8]): usize {
    return bytes.len()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_view_is_empty_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func empty(bytes: &[u8]): bool {
    return bytes.is_empty()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_view_len_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func size(bytes: &+[u8]): usize {
    return bytes.len()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_view_is_empty_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func empty(bytes: &+[u8]): bool {
    return bytes.is_empty()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_str_index_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
fn accepts_str_len_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func size(): usize {
    return "hello".len()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_str_is_empty_call_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func empty(): bool {
    return "".is_empty()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_index_on_non_indexable_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
        r#"func main(): i32 {
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
fn accepts_indexing_through_one_readonly_coercion() {
    let diagnostics = check_text(
        r#"struct Buffer {
    values: &[u8]
}

instance Buffer {
    pub coerce &self as &[u8] from self {
        return self.values
    }
}

func first(buffer: &Buffer): u8 {
    return buffer[0]
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_generic_index_requirement_and_checks_concrete_coercion() {
    let diagnostics = check_text(
        r#"struct Buffer {
    values: &[u8]
}

instance Buffer {
    pub coerce &self as &[u8] from self {
        return self.values
    }
}

func at<C, K, V>(container: &C, index: K, marker: &V): &V from container where (&C[K]): &V {
    return &container[index]
}

func check(bytes: &[u8]): void {
    let buffer = Buffer { values: bytes }
    let marker: u8 = 0
    let value = at(&buffer, 0, &marker)
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unsatisfied_generic_index_requirement() {
    let diagnostics = check_text(
        r#"struct Scalar { value: i32 }

func at<C, K, V>(container: &C, index: K, marker: &V): &V from container where (&C[K]): &V {
    return &container[index]
}

func main(): i32 {
    let scalar = Scalar { value: 0 }
    let marker: u8 = 0
    let value = at(&scalar, 0, &marker)
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0475"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_ambiguous_index_coercion_targets() {
    let diagnostics = check_text(
        r#"struct Buffer {
    bytes: &[u8]
    words: &[usize]
}

instance Buffer {
    pub coerce &self as &[u8] from self { return self.bytes }
    pub coerce &self as &[usize] from self { return self.words }
}

func first(buffer: &Buffer): u8 {
    return buffer[0]
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0476"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_ambiguity_between_declared_and_primitive_coercion_targets() {
    let diagnostics = check_text(
        r#"struct Indexed {
    values: &[u8]
}

instance Indexed {
    pub operator (&self[index: usize]): &u8 {
        return &self.values[index]
    }
}

struct Buffer {
    indexed: Indexed
    bytes: &[u8]
}

instance Buffer {
    pub coerce &self as &Indexed from self { return &self.indexed }
    pub coerce &self as &[u8] from self { return self.bytes }
}

func first(buffer: &Buffer): u8 {
    return buffer[0]
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0476"),
        "{diagnostics:?}"
    );
}
