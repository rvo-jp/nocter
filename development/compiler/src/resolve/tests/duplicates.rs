use super::support::resolve_text;

#[test]
fn diagnoses_reserved_type_declaration_name_reuse() {
    for source in [
        r#"type error = i32

func main(): i32 {
    return 0
}
"#,
        r#"struct i32 {
    value: bool
}

func main(): i32 {
    return 0
}
"#,
    ] {
        let output = resolve_text(source);

        assert_eq!(
            output.diagnostics.len(),
            1,
            "{source}\n{:?}",
            output.diagnostics
        );
        assert_eq!(output.diagnostics[0].code, "E0401");
        assert!(
            output.diagnostics[0]
                .message
                .contains("reserved type spelling"),
            "{source}\n{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_builtin_type_value_name_reuse() {
    for source in [
        r#"func i32(): i32 {
    return 0
}

func main(): i32 {
    return 0
}
"#,
        r#"primitive usize(): i32

func main(): i32 {
    return 0
}
"#,
        r#"use std/io as bool

func main(): i32 {
    return 0
}
"#,
        r#"use std/io.print as str

func main(): i32 {
    return 0
}
"#,
    ] {
        let output = resolve_text(source);
        assert_eq!(
            output.diagnostics.len(),
            1,
            "{source}\n{:?}",
            output.diagnostics
        );
        assert_eq!(output.diagnostics[0].code, "E0401");
        assert!(
            output.diagnostics[0].message.contains("value name"),
            "{source}\n{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_reserved_type_generic_parameter_name_reuse() {
    for source in [
        r#"func identity<i32>(value: i32): i32 {
    return value
}

func main(): i32 {
    return identity(1)
}
"#,
        r#"primitive syscall<usize>(number: usize): i32

func main(): i32 {
    return 0
}
"#,
        r#"type Alias<error> = i32

func main(): i32 {
    return 0
}
"#,
        r#"struct Box<str> {
    value: i32
}

func main(): i32 {
    return 0
}
"#,
        r#"enum Choice<bool> {
    ready
}

func main(): i32 {
    return 0
}
"#,
        r#"interface Source<u8> {
    pub method &self.get(): i32
}

func main(): i32 {
    return 0
}
"#,
    ] {
        let output = resolve_text(source);
        assert_eq!(
            output.diagnostics.len(),
            1,
            "{source}\n{:?}",
            output.diagnostics
        );
        assert_eq!(output.diagnostics[0].code, "E0401");
        assert!(
            output.diagnostics[0]
                .message
                .contains("reserved type spelling"),
            "{source}\n{:?}",
            output.diagnostics
        );
        assert!(
            output.diagnostics[0].message.contains("generic parameter"),
            "{source}\n{:?}",
            output.diagnostics
        );
    }
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
fn diagnoses_duplicate_inherent_method_names_across_instances() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

instance File {
    method self.name(): i32 {
        return self.fd
    }
}

instance File {
    method self.name(value: i32): i32 {
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

instance File {
    method self.open(): i32 {
        return self.fd
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

instance AppError {
    method self.missing_path(): i32 {
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
fn diagnoses_duplicate_destruct_declarations() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

destruct File(&+self) {
    return
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
fn diagnoses_duplicate_function_parameter_names() {
    let output = resolve_text(
        r#"func add(value: i32, value: i32): i32 {
    return value
}

func main(): i32 {
    return add(1, 2)
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
    assert!(output.diagnostics[0].message.contains("function `add`"));
}

#[test]
fn diagnoses_duplicate_associated_function_parameter_names() {
    let output = resolve_text(
        r#"struct Counter {
    value: i32
}

construct Counter {
    pub default func new(value: i32, value: i32): Self {
        return Counter { value: value }
    }
}

func main(): i32 {
    let counter = Counter.new(1, 2)
    return counter.value
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
    assert!(
        output.diagnostics[0]
            .message
            .contains("function `Counter.new`")
    );
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
fn repeated_declaration_pattern_binders_share_one_identity() {
    let output = resolve_text(
        r#"struct Pair<L, R> {
    left: L
    right: R
}

instance Pair<T, T> {
    method self.value(): T {
        return self.left
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn diagnoses_duplicate_interface_method_names() {
    let output = resolve_text(
        r#"interface Writer {
    pub method &self.write(text: &str): void
    pub method &self.write(bytes: &[u8]): void
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
    pub method &self.write(text: &str, text: &str): void
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
    pub method &self.write(self: &str): void
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
            .contains("parameter named `self`")
    );
}

#[test]
fn diagnoses_inherent_method_parameter_reusing_receiver_name_once() {
    let output = resolve_text(
        r#"struct Counter {
    value: i32
}

instance Counter {
    method &self.add(self: i32): i32 {
        return self
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0421");
    assert!(output.diagnostics[0].message.contains("method `add`"));
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
