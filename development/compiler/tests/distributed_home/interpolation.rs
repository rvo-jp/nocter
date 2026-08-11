use super::*;

#[test]
fn distributed_std_owned_interpolation_checks() {
    let project = TempProject::new("distributed-home-interpolation-check");
    let source = project.write_source(
        "interpolation_check.nct",
        r#"func render(value: usize): String {
    return "value ${value}"
}

func main(): i32 {
    let bare: &str = "static"
    let owned: String = "${bare} ${true}"
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_formats_decoded_text_values_and_owned_strings_in_order() {
    let project = TempProject::new("distributed-home-interpolation-runtime");
    let source = project.write_source(
        "interpolation_runtime.nct",
        r#"use std/io.print

func marked(label: &str, value: i32): i32! {
    print(label)?
    return value
}

func temporary(): String {
    return "temporary ${7}"
}

func main(): i32! {
    let existing = String "owned"
    let byte: u8 = 255
    let word: usize = 18446744073709551615
    let i8_min: i8 = -128
    let i16_min: i16 = -32768
    let i64_min: i64 = -9223372036854775808
    let signed_size: isize = -9
    let u16_max: u16 = 65535
    let u32_max: u32 = 4294967295
    let u64_max: u64 = 18446744073709551615
    let text = """
        escaped \"line\"\n${marked("A", -2147483648)?}/${marked("B", 0)?}/${marked("C", 2147483647)?}
        ${byte}/${word}/${false}/${existing}/${temporary()}
        ${i8_min}/${i16_min}/${i64_min}/${signed_size}/${u16_max}/${u32_max}/${u64_max}
        """
    print((&text as &str))?
    if (&existing as &str) != "owned" {
        return 1
    }
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(
        text(&output.stdout),
        "ABCescaped \"line\"\n-2147483648/0/2147483647\n255/18446744073709551615/false/owned/temporary 7\n-128/-32768/-9223372036854775808/-9/65535/4294967295/18446744073709551615"
    );
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_is_an_ordinary_owned_aggregate_value() {
    let project = TempProject::new("distributed-home-interpolation-contexts");
    let source = project.write_source(
        "interpolation_contexts.nct",
        r#"struct Holder {
    text: String
}

func consume(text: String): i32 {
    if (&text as &str) != "argument 2" {
        return 1
    }
    return 0
}

func rendered(): String {
    return "return ${3}"
}

func main(): i32 {
    var assigned = String "initial"
    assigned = "assigned ${1}"
    if (&assigned as &str) != "assigned 1" {
        return 2
    }

    let holder = Holder { text: "field ${4}" }
    if (&holder.text as &str) != "field 4" {
        return 3
    }
    if (&rendered() as &str) != "return 3" {
        return 4
    }
    return consume("argument ${2}")
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_uses_and_releases_lexical_region_context() {
    let project = TempProject::new("distributed-home-region-interpolation");
    let source = project.write_source(
        "region_interpolation.nct",
        r#"use std/mem.page_allocator

func main(): i32 {
    let arena = page_allocator()
    region temporary using arena {
        let text = "region ${42}"
        if (&text as &str) != "region 42" {
            return 1
        }
    }
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_allocation_failure_aborts_without_unwinding() {
    let project = TempProject::new("distributed-home-interpolation-allocation-abort");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);
    let mem_module = home.join("std/mem/index.nct");
    let mem_source = fs::read_to_string(&mem_module).unwrap();
    let original = r#"pub(/) func try_grow_owned(buffer: &+RawBuffer, new_size: usize): void! {
    var allocator = TryAllocator {
        state: buffer.allocator_state,
        kind: buffer.allocator_kind,
    }
    if buffer.allocator_kind == 2 {
        allocator.state = current_allocator_state()
        allocator.kind = current_allocator_kind()
    }
    try_grow(&+allocator, buffer, new_size)?
    return
}"#;
    let failing = r#"pub(/) func try_grow_owned(buffer: &+RawBuffer, new_size: usize): void! {
    return error.new("test.out_of_memory", "deterministic interpolation failure")
}"#;
    assert!(mem_source.contains(original));
    fs::write(&mem_module, mem_source.replace(original, failing)).unwrap();
    let source = project.write_source(
        "interpolation_allocation_abort.nct",
        r#"func main(): i32 {
    let text = "must allocate ${1}"
    return 1
}
"#,
    );

    let output = Command::new(NOCTER)
        .args(["run", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", home)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(70));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn distributed_std_region_backed_interpolation_cannot_escape_through_an_aggregate() {
    let project = TempProject::new("distributed-home-region-interpolation-escape");
    let source = project.write_source(
        "region_interpolation_escape.nct",
        r#"use std/mem.page_allocator

struct Holder {
    text: String
}

func leak(): Holder {
    let arena = page_allocator()
    region temporary using arena {
        return Holder { text: "escape ${42}" }
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("E0436"), "{stderr}");
    assert!(stderr.contains("region `temporary`"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_dispatches_to_a_user_format_conformance() {
    let project = TempProject::new("distributed-home-user-format-runtime");
    let source = project.write_source(
        "user_format_runtime.nct",
        r#"use std/fmt.{Format, append_i32, append_str}
use std/io.print

struct Point {
    x: i32
    y: i32
}

conform Format for Point {
    method &self.format_into(output: &+String): void {
        append_str(output, "(")
        append_i32(output, self.x)
        append_str(output, ", ")
        append_i32(output, self.y)
        append_str(output, ")")
        return
    }
}

func render<T>(value: &T): String where T: Format {
    return "generic ${value}"
}

func main(): i32! {
    let point = Point { x: 3, y: 4 }
    let text = render(&point)
    print((&text as &str))?
    if point.x != 3 {
        return 1
    }
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"generic (3, 4)");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_discovers_an_imported_type_conformance() {
    let project = TempProject::new("distributed-home-imported-format-runtime");
    project.write_source(
        "point/index.nct",
        r#"use std/fmt.{Format, append_i32, append_str}

pub struct Point {
    x: i32
    y: i32
}

construct Point {
    pub default func new(x: i32, y: i32): Self {
        return Point { x: x, y: y }
    }
}

conform Format for Point {
    method &self.format_into(output: &+String): void {
        append_str(output, "Point(")
        append_i32(output, self.x)
        append_str(output, ", ")
        append_i32(output, self.y)
        append_str(output, ")")
        return
    }
}
"#,
    );
    let source = project.write_source(
        "index.nct",
        r#"use ./point.Point
use std/io.print

func main(): i32! {
    let point = Point.new(8, 13)
    let text = "${point}"
    print((&text as &str))?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"Point(8, 13)");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_drops_a_move_only_temporary_after_formatting_once() {
    let project = TempProject::new("distributed-home-temporary-format-drop-runtime");
    let source = project.write_source(
        "index.nct",
        r#"use std/fmt.{Format, append_str}
use std/io.print

struct Token {
    label: &str
}

destruct Token(&+self) {
    print("D")!
    return
}

conform Format for Token {
    method &self.format_into(output: &+String): void {
        append_str(output, self.label)
        return
    }
}

func temporary(): Token {
    return Token { label: "token" }
}

func main(): i32! {
    let text = "${temporary()}"
    print((&text as &str))?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"Dtoken");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn distributed_std_interpolation_requires_the_exact_standard_format_contract() {
    let project = TempProject::new("distributed-home-exact-format-check");
    let source = project.write_source(
        "index.nct",
        r#"interface Format {
    pub method &self.format_into(output: &+String): void
}

struct Point {
    x: i32
}

conform Format for Point {
    method &self.format_into(output: &+String): void {
        return
    }
}

func main(): i32 {
    let point = Point { x: 3 }
    let text = "${point}"
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("E0379"), "{stderr}");
    assert!(stderr.contains("std/fmt.Format"), "{stderr}");
}

#[test]
fn project_code_cannot_define_builtin_type_conformances() {
    let project = TempProject::new("distributed-home-builtin-conformance-authority");
    let source = project.write_source(
        "index.nct",
        r#"interface ProjectFormat {
    pub method &self.render(): void
}

conform ProjectFormat for i32 {
    method &self.render(): void {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("E0416"), "{stderr}");
    assert!(stderr.contains("built-in type"), "{stderr}");
}

#[test]
fn distributed_lsp_exposes_interpolation_hover_completion_and_signature_recovery() {
    let project = TempProject::new("distributed-home-interpolation-lsp");
    let hover_text = r#"func format(value: i32): i32 {
    return value
}

func main(count: i32): i32 {
    let text = "value ${format(count)}"
    return 0
}
"#;
    let completion_text = r#"func main(count: i32): i32 {
    let text = "value ${cou
    return 0
}
"#;
    let signature_text = r#"func format(value: i32): i32 {
    return value
}

func main(): i32 {
    let text = "value ${format(
    return 0
}
"#;
    let source = project.write_source("interpolation_lsp.nct", hover_text);
    let uri = file_uri(&source);
    let hover_offset = hover_text.find("value ${").unwrap();
    let nested_hover_offset = hover_text.rfind("format(count)").unwrap();
    let completion_offset = completion_text.find("cou\n").unwrap() + 3;
    let signature_offset = signature_text.find("format(\n").unwrap() + "format(".len();
    let output = nocter_lsp(
        &distributed_home().join("nocter"),
        project.root(),
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":uri,"languageId":"nocter","version":1,"text":hover_text}}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":position(hover_text, hover_offset)}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":6,
                "method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":position(hover_text, nested_hover_offset)}
            }),
            json!({
                "jsonrpc":"2.0",
                "method":"textDocument/didChange",
                "params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":completion_text}]}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"textDocument/completion",
                "params":{"textDocument":{"uri":uri},"position":position(completion_text, completion_offset)}
            }),
            json!({
                "jsonrpc":"2.0",
                "method":"textDocument/didChange",
                "params":{"textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":signature_text}]}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"textDocument/signatureHelp",
                "params":{"textDocument":{"uri":uri},"position":position(signature_text, signature_offset)}
            }),
            json!({"jsonrpc":"2.0","id":5,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let frames = read_frames(&output.stdout);
    let hover = response_with_id(&frames, 2);
    let hover_markdown = hover["result"]["contents"]["value"]
        .as_str()
        .expect("expected interpolation hover markdown");
    assert!(hover_markdown.contains("interpolated string: String"));
    assert!(!hover_markdown.contains("Allocation effect"));
    assert!(!hover_markdown.contains("Result provenance"));
    assert!(hover_markdown.contains("Accepted interpolation input:** `&str`"));
    assert!(hover_markdown.contains("Formatting contract:** `Format`"));

    let nested_hover = response_with_id(&frames, 6);
    let nested_hover_markdown = nested_hover["result"]["contents"]["value"]
        .as_str()
        .expect("expected nested expression hover markdown");
    assert!(nested_hover_markdown.contains("func format(value: i32): i32"));
    assert!(!nested_hover_markdown.contains("interpolated string"));

    let completion = response_with_id(&frames, 3);
    let labels = completion["result"]["items"]
        .as_array()
        .expect("expected completion items")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"count"), "completion labels: {labels:?}");

    let signature = response_with_id(&frames, 4);
    assert_eq!(
        signature["result"]["signatures"][0]["label"],
        "func format(value: i32): i32"
    );
}

fn position(text: &str, offset: usize) -> Value {
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    json!({"line":line,"character":text[line_start..offset].chars().count()})
}

fn response_with_id(frames: &[Value], id: u64) -> &Value {
    frames
        .iter()
        .find(|message| message["id"] == id)
        .unwrap_or_else(|| panic!("missing response {id}: {frames:#?}"))
}
