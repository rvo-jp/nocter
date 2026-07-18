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
fn diagnoses_duplicate_struct_field_names() {
    let output = resolve_text(
        r#"struct Packet {
    len: i32
    len: i32
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0417");
}

#[test]
fn diagnoses_duplicate_enum_variant_names() {
    let output = resolve_text(
        r#"enum AppError {
    missing_path
    missing_path
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0418");
}

#[test]
fn diagnoses_duplicate_enum_variant_payload_names() {
    let output = resolve_text(
        r#"enum AppError {
    open_failed(path: &str, path: i32)
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0419");
}

#[test]
fn diagnoses_duplicate_function_generic_parameter_names() {
    let output = resolve_text(
        r#"func identity<T, T>(value: i32): i32 {
    return value
}

func main(): i32 {
    return identity(1)
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0420");
}

#[test]
fn diagnoses_duplicate_struct_generic_parameter_names() {
    let output = resolve_text(
        r#"struct Box<T, T> {
    value: i32
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0420");
}

#[test]
fn diagnoses_duplicate_impl_generic_parameter_names() {
    let output = resolve_text(
        r#"struct Box<T> {
    value: T
}

impl<T, T> Box<T> {
    method (box: Self).value(): T {
        return box.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0420");
}

#[test]
fn diagnoses_duplicate_interface_method_names() {
    let output = resolve_text(
        r#"interface Writer {
    pub method (writer: &Self).write(text: &str): void
    pub method (writer: &Self).write(bytes: &[u8]): void
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0428");
    assert!(output.diagnostics[0].message.contains("interface `Writer`"));
    assert!(
        output.diagnostics[0]
            .message
            .contains("method named `write`")
    );
}

#[test]
fn diagnoses_duplicate_interface_method_parameter_names() {
    let output = resolve_text(
        r#"interface Writer {
    pub method (writer: &Self).write(text: &str, text: &str): void
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
    assert!(
        output.diagnostics[0]
            .message
            .contains("interface method `Writer.write`")
    );
}

#[test]
fn diagnoses_interface_method_parameter_reusing_receiver_name() {
    let output = resolve_text(
        r#"interface Writer {
    pub method (writer: &Self).write(writer: &str): void
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
    assert!(
        output.diagnostics[0]
            .message
            .contains("parameter named `writer`")
    );
}

#[test]
fn diagnoses_duplicate_primitive_parameter_names() {
    let output = resolve_text(
        r#"primitive syscall(fd: i32, fd: i32): i32

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
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
