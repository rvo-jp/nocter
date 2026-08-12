use super::*;

#[test]
fn distributed_std_text_and_collection_surface_passes_check() {
    let project = TempProject::new("distributed-home-practical-std-check");
    let source = project.write_source(
        "practical_std_shape.nct",
        r#"use std/string.is_valid_utf8
use std/vec.Vec

func main(): i32 {
    let bytes: Vec<u8> = Vec [104, 105]
    let valid: bool = is_valid_utf8((&bytes as &[u8]))
    let position: usize = "hello".find("ell") otherwise { return 1 }
    let found: bool = "hello".contains("ell") && "hello".starts_with("he") && "hello".ends_with("lo")
    var parts: Vec<String> = "a::b".split("::") catch failure { return 2 }
    var values = Vec [1, 2, 3]
    values.retain((value) { value != 2 })
    values.retain((value) { value == 1 })
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_path_io_numeric_and_process_surface_passes_check() {
    let project = TempProject::new("distributed-home-practical-services-check");
    let source = project.write_source(
        "practical_services_shape.nct",
        r#"use std/io.{File, Reader, Writer}
use std/io/buffer.{BufReader, BufWriter}
use std/num.{i32_to_string, parse_i32, parse_u8, parse_usize, usize_to_string}
use std/path.Utf8Path
use std/process.{arg, arg_count, environment, environment_count}
use std/vec.Vec

func main(): i32 {
    let path = Utf8Path.new("file.txt") catch failure { return 1 }
    let child = path.join("child") catch failure { return 2 }
    let count: usize = parse_usize("42") otherwise { return 3 }
    let signed: i32 = parse_i32("-7") otherwise { return 4 }
    let byte: u8 = parse_u8("8") otherwise { return 5 }
    let first_arg: &str = arg(0) catch failure { return 6 } otherwise { return 7 }
    let process_arg_count: usize = arg_count()
    let process_environment_count: usize = environment_count()
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_whole_stream_io_surface_passes_check() {
    let project = TempProject::new("distributed-home-whole-stream-io-check");
    let source = project.write_source(
        "whole_stream_io_shape.nct",
        r#"use std/io.{File, Reader, Writer}
use std/io/buffer.{BufReader, BufWriter}

func collect<R>(reader: &+R): String! where R: Reader {
    return reader.read_to_string()?
}

func emit<W>(writer: &+W, text: &str): void! where W: Writer {
    writer.write_text(text)?
    writer.flush()?
    return
}

func main(): i32! {
    var input = File.open("input.txt")?
    let bytes = input.read_to_end()?
    let reopened = File.open("input.txt")?
    var buffered = BufReader.new(move reopened)
    let text = collect(&+buffered)?
    let output = File.create("output.txt")?
    var writer = BufWriter.new(move output)
    emit(&+writer, (&text as &str))?
    writer.close()?
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_whole_stream_io_operations_run() {
    let project = TempProject::new("distributed-home-whole-stream-io-run");
    let empty_path = project.root().join("empty.txt");
    let input_path = project.root().join("large.txt");
    let invalid_path = project.root().join("invalid.txt");
    let output_path = project.root().join("output.txt");
    fs::write(&empty_path, []).unwrap();
    let mut expected = vec![b'a'; 8191];
    expected.extend_from_slice("é\n".as_bytes());
    fs::write(&input_path, &expected).unwrap();
    fs::write(&invalid_path, [0xf0, 0x28, 0x8c, 0x28]).unwrap();

    let source_text = r#"use std/io.File
use std/io/buffer.{BufReader, BufWriter}

func rejects_invalid_utf8(path: &str): bool {
    var input = File.open(path) catch failure { return false }
    let text = input.read_to_string() catch failure { return true }
    return false
}

func main(): i32! {
    var empty = File.open("__EMPTY__")?
    let nothing = empty.read_to_end()?
    if nothing.len() != 0 { return 1 }

    var direct = File.open("__INPUT__")?
    let text = direct.read_to_string()?
    if text.len() != 8194 { return 2 }
    let encoded = text.bytes()
    if encoded[8191] != 195 || encoded[8192] != 169 || encoded[8193] != 10 { return 3 }

    let source = File.open("__INPUT__")?
    var buffered = BufReader.with_capacity(move source, 3)
    let collected = buffered.read_to_end()?
    if collected.len() != 8194 { return 4 }
    if (&collected as &[u8])[0] != 97 || (&collected as &[u8])[8192] != 169 { return 5 }

    if !rejects_invalid_utf8("__INVALID__") { return 6 }

    let destination = File.create("__OUTPUT__")?
    var writer = BufWriter.with_capacity(move destination, 2)
    writer.write_text((&text as &str))?
    writer.close()?
    return 42
}
"#
    .replace("__EMPTY__", empty_path.to_str().unwrap())
    .replace("__INPUT__", input_path.to_str().unwrap())
    .replace("__INVALID__", invalid_path.to_str().unwrap())
    .replace("__OUTPUT__", output_path.to_str().unwrap());
    let source = project.write_source("whole_stream_io_run.nct", &source_text);

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(fs::read(output_path).unwrap(), expected);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_whole_stream_rejects_reader_contract_violations_and_failures() {
    let project = TempProject::new("distributed-home-whole-stream-reader-errors");
    let source = project.write_source(
        "whole_stream_reader_errors.nct",
        r#"use std/io.Reader

struct InvalidCountReader {
    called: bool
}

conform Reader for InvalidCountReader {
    method &+self.read(buffer: &+[u8]): usize! from static {
        self.called = true
        return buffer.len() + 1
    }
}

struct FailingReader {
    calls: usize
}

conform Reader for FailingReader {
    method &+self.read(buffer: &+[u8]): usize! from static {
        if self.calls == 0 {
            self.calls = 1
            buffer[0] = 65
            return 1
        }
        return error.new("test.read_failed", "read failed")
    }
}

func rejects_invalid_count(): bool {
    var reader = InvalidCountReader { called: false }
    let collected = reader.read_to_end() catch failure { return reader.called }
    return false
}

func propagates_read_failure(): bool {
    var reader = FailingReader { calls: 0 }
    let collected = reader.read_to_end() catch failure { return reader.calls == 1 }
    return false
}

func main(): i32 {
    if !rejects_invalid_count() { return 1 }
    if !propagates_read_failure() { return 2 }
    return 42
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

    let uncaught = project.write_source(
        "invalid_count_uncaught.nct",
        r#"use std/io.Reader

struct InvalidCountReader {
    called: bool
}

conform Reader for InvalidCountReader {
    method &+self.read(buffer: &+[u8]): usize! from static {
        return buffer.len() + 1
    }
}

func main(): i32! {
    var reader = InvalidCountReader { called: false }
    let collected = reader.read_to_end()?
    return 0
}
"#,
    );
    let uncaught_output = nocter_run(&project, &uncaught);
    assert_eq!(uncaught_output.status.code(), Some(1));
    let stderr = text(&uncaught_output.stderr);
    assert!(
        stderr.contains("std.io.invalid_read_count")
            && stderr.contains("more bytes than the supplied buffer"),
        "{stderr}"
    );
}

#[test]
fn distributed_lsp_presents_whole_stream_defaults_for_concrete_receivers() {
    let project = TempProject::new("distributed-home-whole-stream-lsp");
    let source_text = r#"use std/io.File

func main(): i32! {
    var input = File.open("input.txt")?
    let text = input.read_to_string()?
    return 0
}
"#;
    let source = project.write_source("whole_stream_lsp.nct", source_text);
    let uri = file_uri(&source);
    let member_offset = source_text.find("read_to_string").unwrap();
    let completion_offset = source_text.find("input.read_to_string").unwrap() + "input.".len();
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
                "jsonrpc":"2.0",
                "id":2,
                "method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":io_lsp_position(source_text,member_offset)}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"textDocument/definition",
                "params":{"textDocument":{"uri":uri},"position":io_lsp_position(source_text,member_offset)}
            }),
            json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"textDocument/completion",
                "params":{"textDocument":{"uri":uri},"position":io_lsp_position(source_text,completion_offset)}
            }),
            json!({"jsonrpc":"2.0","id":5,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let frames = read_frames(&output.stdout);
    let hover = io_lsp_response(&frames, 2)["result"]["contents"]["value"]
        .as_str()
        .expect("expected whole-stream method hover");
    assert!(
        hover.contains("method &+File.read_to_string(): String!"),
        "{hover}"
    );
    assert!(!hover.contains("std/io."), "{hover}");

    let definition = &io_lsp_response(&frames, 3)["result"];
    let target_uri = definition
        .as_array()
        .and_then(|locations| locations.first())
        .and_then(|location| location["targetUri"].as_str())
        .or_else(|| definition["uri"].as_str());
    assert!(
        target_uri.is_some_and(|uri| uri.ends_with("/std/io/index.nct")),
        "definition: {definition:#?}"
    );

    let completion = io_lsp_response(&frames, 4)["result"]["items"]
        .as_array()
        .expect("expected member completion items");
    for expected in ["read", "read_to_end", "read_to_string"] {
        assert!(
            completion
                .iter()
                .any(|item| item["label"].as_str() == Some(expected)),
            "missing {expected}: {completion:#?}"
        );
    }
}

fn io_lsp_position(text: &str, offset: usize) -> Value {
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    json!({"line":line,"character":text[line_start..offset].chars().count()})
}

fn io_lsp_response(frames: &[Value], id: u64) -> &Value {
    frames
        .iter()
        .find(|message| message["id"] == id)
        .unwrap_or_else(|| panic!("missing response {id}: {frames:#?}"))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_text_and_collection_operations_run() {
    let project = TempProject::new("distributed-home-practical-std-run");
    let source = project.write_source(
        "practical_std_run.nct",
r#"use std/string.is_valid_utf8
use std/vec.Vec

func rejects_invalid_utf8(candidate: &[u8]): bool {
    let accepted = String.from_utf8(candidate) catch failure {
        return true
    }
    return false
}

func rejects_empty_separator(): bool {
    let accepted: Vec<String> = "abc".split("") catch failure {
        return true
    }
    return false
}

func main(): i32 {
    if "hello".bytes().len() != 5 || "e".bytes().len() != 1 { return 19 }
    let left: u8 = "hello".bytes()[1]
    let right: u8 = "e".bytes()[0]
    if left != right { return 18 }
    let position: usize = "hello".find("e") otherwise { return 20 }
    if position != 1 { return 21 }
    if !String "hello".contains("ell") { return 2 }
    if !String "hello".starts_with("he") || !String "hello".ends_with("lo") { return 3 }
    let invalid: Vec<u8> = Vec [240, 40, 140, 40]
    if is_valid_utf8((&invalid as &[u8])) { return 4 }
    if !rejects_invalid_utf8((&invalid as &[u8])) { return 4 }
    let encoded: Vec<u8> = Vec [104, 195, 169]
    let decoded = String.from_utf8((&encoded as &[u8])) catch failure { return 5 }
    if (&decoded as &str) != "hé" { return 6 }

    var parts = "a::b::".split("::") catch failure { return 7 }
    if parts.len() != 3 { return 8 }
    let final_part = parts.pop() otherwise { return 9 }
    let middle_part = parts.pop() otherwise { return 10 }
    let first_part = parts.pop() otherwise { return 11 }
    if (&first_part as &str) != "a" || (&middle_part as &str) != "b" || (&final_part as &str) != "" { return 12 }
    if !rejects_empty_separator() { return 12 }

    var values = Vec [1, 2, 3, 4, 5]
    values.retain((value) { value % 2 != 0 })
    if values.len() != 3 || (&values as &[i32])[0] != 1 || (&values as &[i32])[1] != 3 || (&values as &[i32])[2] != 5 { return 13 }
    return 42
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_filesystem_cli_foundation_runs() {
    let project = TempProject::new("distributed-home-filesystem-cli-run");
    let fixture = project.write_source("input.txt", "alpha\nbeta\n");
    let output_path = project.root().join("output.txt");
    let source_text = r#"use std/io.{File, Reader, Writer, stdout}
use std/io/buffer.{BufReader, BufWriter}
use std/num.{parse_i32, usize_to_string}
use std/path.Utf8Path
use std/process.{arg, arg_count}
use std/vec.Vec

func main(): i32 {
    let path = Utf8Path.new("__PATH__") catch failure { return 1 }
    if !path.is_absolute() { return 2 }
    let file = File.open(&path) catch failure { return 3 }
    var reader = BufReader.with_capacity(move file, 3)
    var buffer: Vec<u8> = Vec [0, 0, 0, 0, 0, 0]
    let received: usize = reader.read((&+buffer as &+[u8])) catch failure { return 4 }
    if received != 6 || (&buffer as &[u8])[0] != 97 || (&buffer as &[u8])[5] != 10 { return 5 }

    let created_path = Utf8Path.new("__OUTPUT__") catch failure { return 14 }
    let created_file = File.create(&created_path) catch failure { return 15 }
    var file_writer = BufWriter.with_capacity(move created_file, 2)
    file_writer.write("written".bytes()) catch failure { return 16 }
    file_writer.close() catch failure { return 17 }
    let reopened = File.open(&created_path) catch failure { return 18 }
    var verifier = BufReader.with_capacity(move reopened, 2)
    var verification: Vec<u8> = Vec [0, 0, 0, 0, 0, 0, 0]
    let verified: usize = verifier.read((&+verification as &+[u8])) catch failure { return 19 }
    if verified != 7 || (&verification as &[u8])[0] != 119 || (&verification as &[u8])[6] != 110 { return 20 }
    let appended_file = File.append(&created_path) catch failure { return 21 }
    var appender = BufWriter.with_capacity(move appended_file, 1)
    appender.write("!".bytes()) catch failure { return 22 }
    appender.close() catch failure { return 23 }

    let number: i32 = parse_i32("-2147483648") otherwise { return 6 }
    if number != -2147483648 || (&usize_to_string(42) as &str) != "42" { return 7 }
    if arg_count() == 0 { return 8 }
    let executable: &str = arg(0) catch failure { return 9 } otherwise { return 10 }
    if executable.len() == 0 { return 11 }

    var writer = BufWriter.with_capacity(stdout(), 2)
    writer.write("ok".bytes()) catch failure { return 12 }
    writer.flush() catch failure { return 13 }
    return 42
}
"#
    .replace("__PATH__", fixture.to_str().unwrap())
    .replace("__OUTPUT__", output_path.to_str().unwrap());
    let source = project.write_source("filesystem_cli_run.nct", &source_text);

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(text(&output.stdout), "ok");
    assert_eq!(fs::read(&output_path).unwrap(), b"written!");
}
