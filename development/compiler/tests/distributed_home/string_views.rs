use super::*;

#[test]
fn distributed_std_borrowed_text_view_surface_passes_check() {
    let project = TempProject::new("distributed-home-borrowed-text-view-check");
    let source = project.write_source(
        "borrowed_text_view_shape.nct",
        r#"use std/iter.Iterator
use std/string.{LinesIter, SplitIter, get_range, lines, split_views, strip_prefix, strip_suffix}

func first_part(text: &str): &str! {
    var parts: SplitIter = split_views(text, ",")?
    return parts.next() otherwise { return text }
}

func first_line(text: &str): &str {
    var source: LinesIter = lines(text)
    return source.next() otherwise { return text }
}

func main(): i32 {
    let range: &str = get_range("hello", 1, 4) otherwise { return 1 }
    let prefix: &str = strip_prefix("hello", "he") otherwise { return 2 }
    let suffix: &str = strip_suffix("hello", "lo") otherwise { return 3 }
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_borrowed_text_ranges_and_iterators_run() {
    let project = TempProject::new("distributed-home-borrowed-text-view-run");
    let source = project.write_source(
        "borrowed_text_view_run.nct",
        r#"use std/string.{get_range, is_char_boundary, lines, split, split_views, strip_prefix, strip_suffix}

func main(): i32! {
    let utf8: &str = "aé日😀z"
    if !is_char_boundary(utf8, 0) || !is_char_boundary(utf8, 1) { return 1 }
    if is_char_boundary(utf8, 2) || !is_char_boundary(utf8, 3) { return 2 }
    if is_char_boundary(utf8, 4) || is_char_boundary(utf8, 5) || !is_char_boundary(utf8, 6) { return 3 }
    if is_char_boundary(utf8, 7) || is_char_boundary(utf8, 8) || is_char_boundary(utf8, 9) || !is_char_boundary(utf8, 10) { return 4 }
    if !is_char_boundary(utf8, 11) || is_char_boundary(utf8, 12) { return 5 }

    let full = get_range(utf8, 0, 11) otherwise { return 6 }
    let middle = get_range(utf8, 1, 10) otherwise { return 7 }
    let empty = get_range(utf8, 3, 3) otherwise { return 8 }
    if full != utf8 || middle != "é日😀" || empty != "" { return 9 }
    let invalid_boundary = get_range(utf8, 2, 3)
    let reversed = get_range(utf8, 3, 2)
    let out_of_bounds = get_range(utf8, 0, 12)
    if has_value(invalid_boundary) || has_value(reversed) || has_value(out_of_bounds) { return 10 }

    let without_prefix = strip_prefix("prefix-value", "prefix-") otherwise { return 11 }
    let without_suffix = strip_suffix("value-suffix", "-suffix") otherwise { return 12 }
    if without_prefix != "value" || without_suffix != "value" { return 13 }
    let missing_prefix = strip_prefix("value", "x")
    let missing_suffix = strip_suffix("value", "x")
    if has_value(missing_prefix) || has_value(missing_suffix) { return 14 }
    if strip_prefix("value", "")! != "value" || strip_suffix("value", "")! != "value" { return 15 }

    var parts = split_views(",a,,é,😀,", ",")?
    if parts.next()! != "" { return 16 }
    if parts.next()! != "a" { return 17 }
    if parts.next()! != "" { return 18 }
    if parts.next()! != "é" { return 19 }
    if parts.next()! != "😀" { return 20 }
    if parts.next()! != "" { return 21 }
    let exhausted_parts = parts.next()
    if has_value(exhausted_parts) { return 22 }

    var empty_parts = split_views("", ",")?
    if empty_parts.next()! != "" { return 28 }
    let empty_exhausted = empty_parts.next()
    if has_value(empty_exhausted) { return 28 }
    var absent_parts = split_views("whole", ",")?
    if absent_parts.next()! != "whole" { return 29 }
    let absent_exhausted = absent_parts.next()
    if has_value(absent_exhausted) { return 29 }
    var unicode_parts = split_views("a😀b😀", "😀")?
    if unicode_parts.next()! != "a" || unicode_parts.next()! != "b" || unicode_parts.next()! != "" { return 30 }
    if !rejects_empty_separator() { return 31 }

    var adapted = split_views("zero,one,two,three", ",")?.skip(1).take(2)
    if adapted.next()! != "one" || adapted.next()! != "two" { return 32 }
    let adapted_exhausted = adapted.next()
    if has_value(adapted_exhausted) { return 32 }

    var owned = split("a😀b😀", "😀")?
    let owned_last = owned.pop() otherwise { return 33 }
    let owned_middle = owned.pop() otherwise { return 34 }
    let owned_first = owned.pop() otherwise { return 35 }
    if (&owned_first as &str) != "a" || (&owned_middle as &str) != "b" || (&owned_last as &str) != "" { return 36 }

    var text_lines = lines("first\r\n\nthird\rfour\n")
    if text_lines.next()! != "first" { return 23 }
    if text_lines.next()! != "" { return 24 }
    if text_lines.next()! != "third\rfour" { return 25 }
    let exhausted_lines = text_lines.next()
    if has_value(exhausted_lines) { return 26 }
    var empty_lines = lines("")
    let no_empty_line = empty_lines.next()
    if has_value(no_empty_line) { return 27 }

    return 42
}

func has_value(value: &str?): bool {
    let present = value otherwise { return false }
    return true
}

func rejects_empty_separator(): bool {
    let parts = split_views("value", "") catch error { return true }
    return false
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_borrowed_text_iterators_do_not_reach_the_allocator() {
    let project = TempProject::new("distributed-home-borrowed-text-no-allocation");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);

    let mem_module = home.join("std/mem/index.nct");
    let mem_source = fs::read_to_string(&mem_module).unwrap();
    let original = r#"pub func alloc(
    allocator: &+Allocator,
    size: usize,
    align: usize,
): RawBuffer {
    var fallible = TryAllocator { state: allocator.state, kind: allocator.kind }
    return try_alloc(&+fallible, size, align) catch allocation_error {
        return allocation_abort_raw()
    }
}"#;
    let failing = r#"pub func alloc(
    allocator: &+Allocator,
    size: usize,
    align: usize,
): RawBuffer {
    return allocation_abort_raw()
}"#;
    assert!(mem_source.contains(original));
    fs::write(&mem_module, mem_source.replace(original, failing)).unwrap();

    let source = project.write_source(
        "borrowed_text_no_allocation.nct",
        r#"use std/string.{lines, split_views}

func main(): i32! {
    var parts = split_views("zero,one,two,three", ",")?.skip(1).take(2)
    if parts.next()! != "one" || parts.next()! != "two" { return 1 }
    var text_lines = lines("first\r\nsecond\n")
    if text_lines.next()! != "first" || text_lines.next()! != "second" { return 2 }
    return 42
}
"#,
    );

    let output = Command::new(NOCTER)
        .args(["run", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", home)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn distributed_std_borrowed_text_view_keeps_owned_source_loan_active() {
    let project = TempProject::new("distributed-home-borrowed-text-view-loan");
    let source = project.write_source(
        "borrowed_text_view_loan.nct",
        r#"use std/string.{String, get_range}

func invalid(): usize {
    var text = String "hello"
    let view = get_range((&text as &str), 1, 4) otherwise { return 0 }
    text.push_str("!")
    return view.len()
}

func main(): i32 { return 0 }
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0434]"), "{stderr}");
    assert!(
        stderr.contains("text") && stderr.contains("view"),
        "{stderr}"
    );
}

#[test]
fn distributed_std_borrowed_text_iterators_keep_both_inputs_active() {
    let project = TempProject::new("distributed-home-borrowed-text-iterator-loans");
    let source = project.write_source(
        "borrowed_text_iterator_loans.nct",
        r#"use std/string.{String, split_views}

func mutate_text(): usize! {
    var text = String "a,b"
    var parts = split_views((&text as &str), ",")?
    text.push_str(",c")
    return parts.next()!.len()
}

func mutate_separator(): usize! {
    var separator = String ","
    var parts = split_views("a,b", (&separator as &str))?
    separator.push_str(";")
    return parts.next()!.len()
}

func main(): i32 { return 0 }
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0434]").count(), 2, "{stderr}");
    assert!(
        stderr.contains("text") && stderr.contains("separator"),
        "{stderr}"
    );
}

#[test]
fn distributed_std_borrowed_text_view_rejects_source_move_and_drop() {
    let project = TempProject::new("distributed-home-borrowed-text-move-drop");
    let source = project.write_source(
        "borrowed_text_move_drop.nct",
        r#"use std/string.{String, get_range}

func move_source(): usize {
    let text = String "hello"
    let view = get_range((&text as &str), 1, 4) otherwise { return 0 }
    let consumed = move text
    return view.len()
}

func drop_source(): usize {
    let text = String "hello"
    let view = get_range((&text as &str), 1, 4) otherwise { return 0 }
    drop text
    return view.len()
}

func main(): i32 { return 0 }
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0434]").count(), 2, "{stderr}");
}

#[test]
fn distributed_std_borrowed_text_projection_keeps_only_real_origins() {
    let project = TempProject::new("distributed-home-borrowed-text-exact-origins");
    let source = project.write_source(
        "borrowed_text_exact_origins.nct",
        r#"use std/mem.page_allocator
use std/string.{get_range, strip_prefix}

func static_range(): &str {
    return get_range("hello", 1, 4) otherwise { return "" }
}

func temporary_prefix(text: &str): &str {
    var arena = page_allocator()
    region temporary using arena {
        let prefix = String "he"
        return strip_prefix(text, (&prefix as &str)) otherwise { return text }
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_lsp_presents_borrowed_text_views_from_the_public_facade() {
    let project = TempProject::new("distributed-home-borrowed-text-lsp");
    let source_text = r#"use std/string.{get_range, split_views}

func main(): i32! {
    let view = get_range("hello", 1, 4) otherwise { return 1 }
    var parts = split_views("a,b", ",")?
    let first = parts.next() otherwise { return 2 }
    return 0
}
"#;
    let source = project.write_source("borrowed_text_lsp.nct", source_text);
    let uri = file_uri(&source);
    let range_offset = source_text.find("get_range(\"").unwrap();
    let split_offset = source_text.find("split_views(\"").unwrap();
    let signature_offset = source_text.find("1, 4").unwrap() + "1, ".len();
    let completion_offset = source_text.find("parts.next").unwrap() + "parts.".len();
    let output = nocter_lsp(
        &distributed_home().join("nocter"),
        project.root(),
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":uri,"languageId":"nocter","version":1,"text":source_text}}
            }),
            json!({
                "jsonrpc":"2.0","id":2,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":string_view_lsp_position(source_text,range_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":3,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":string_view_lsp_position(source_text,split_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":4,"method":"textDocument/signatureHelp",
                "params":{"textDocument":{"uri":uri},"position":string_view_lsp_position(source_text,signature_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":5,"method":"textDocument/definition",
                "params":{"textDocument":{"uri":uri},"position":string_view_lsp_position(source_text,range_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":6,"method":"textDocument/completion",
                "params":{"textDocument":{"uri":uri},"position":string_view_lsp_position(source_text,completion_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":7,"method":"textDocument/semanticTokens/full",
                "params":{"textDocument":{"uri":uri}}
            }),
            json!({"jsonrpc":"2.0","id":8,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let frames = read_frames(&output.stdout);

    let range_hover = string_view_lsp_response(&frames, 2)["result"]["contents"]["value"]
        .as_str()
        .expect("expected get_range hover");
    assert!(
        range_hover.contains("func get_range(text: &str, start: usize, end: usize): &str?"),
        "{range_hover}"
    );
    assert!(!range_hover.contains(" from "), "{range_hover}");

    let split_hover = string_view_lsp_response(&frames, 3)["result"]["contents"]["value"]
        .as_str()
        .expect("expected split_views hover");
    assert!(
        split_hover.contains(
            "func split_views(text: &str, separator: &str): SplitIter! from text | separator"
        ),
        "{split_hover}"
    );
    assert!(!split_hover.contains("std/string_views."), "{split_hover}");

    let signature = string_view_lsp_response(&frames, 4)["result"]["signatures"][0]["label"]
        .as_str()
        .expect("expected get_range signature help");
    assert_eq!(
        signature,
        "func get_range(text: &str, start: usize, end: usize): &str?"
    );

    let definition = &string_view_lsp_response(&frames, 5)["result"];
    let target_uri = definition
        .as_array()
        .and_then(|locations| locations.first())
        .and_then(|location| location["targetUri"].as_str())
        .or_else(|| definition["uri"].as_str());
    assert!(
        target_uri.is_some_and(|uri| uri.ends_with("/std/string_views/index.nct")),
        "definition: {definition:#?}"
    );

    let completion = string_view_lsp_response(&frames, 6)["result"]["items"]
        .as_array()
        .expect("expected iterator member completion");
    for expected in ["next", "skip", "take"] {
        assert!(
            completion
                .iter()
                .any(|item| item["label"].as_str() == Some(expected)),
            "missing {expected}: {completion:#?}"
        );
    }

    let semantic_data = string_view_lsp_response(&frames, 7)["result"]["data"]
        .as_array()
        .expect("expected semantic tokens");
    let range_position = string_view_lsp_position(source_text, range_offset);
    assert!(
        string_view_semantic_data_contains(
            semantic_data,
            range_position["line"].as_u64().unwrap() as usize,
            range_position["character"].as_u64().unwrap() as usize,
            "get_range".len(),
        ),
        "semantic data: {semantic_data:#?}"
    );
}

fn string_view_lsp_position(text: &str, offset: usize) -> Value {
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    json!({"line":line,"character":text[line_start..offset].chars().count()})
}

fn string_view_lsp_response(frames: &[Value], id: u64) -> &Value {
    frames
        .iter()
        .find(|message| message["id"] == id)
        .unwrap_or_else(|| panic!("missing response {id}: {frames:#?}"))
}

fn string_view_semantic_data_contains(
    data: &[Value],
    expected_line: usize,
    expected_character: usize,
    expected_length: usize,
) -> bool {
    let mut line = 0usize;
    let mut character = 0usize;
    for token in data.chunks_exact(5) {
        let delta_line = token[0].as_u64().unwrap() as usize;
        let delta_character = token[1].as_u64().unwrap() as usize;
        if delta_line == 0 {
            character += delta_character;
        } else {
            line += delta_line;
            character = delta_character;
        }
        if line == expected_line
            && character == expected_character
            && token[2].as_u64() == Some(expected_length as u64)
        {
            return true;
        }
    }
    false
}

#[test]
fn distributed_std_borrowed_text_view_cannot_escape_local_owner() {
    let project = TempProject::new("distributed-home-borrowed-text-view-local-owner");
    let source = project.write_source(
        "borrowed_text_view_region.nct",
        r#"use std/mem.page_allocator
use std/string.get_range

func leak(): &str {
    var arena = page_allocator()
    region temporary using arena {
        let text = String "hello"
        return get_range((&text as &str), 1, 4) otherwise { return "" }
    }
}

func main(): i32 { return 0 }
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0433]"), "{stderr}");
    assert!(stderr.contains("local binding `text`"), "{stderr}");
}
