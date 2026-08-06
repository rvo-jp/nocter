use super::*;

#[test]
fn distributed_std_text_and_collection_surface_passes_check() {
    let project = TempProject::new("distributed-home-practical-std-check");
    let source = project.write_source(
        "practical_std_shape.nct",
        r#"use std/string.{contains, ends_with, find, is_valid_utf8, split, starts_with}
use std/vec.{Vec, retain}

func main(): i32 {
    let bytes: Vec<u8> = Vec [104, 105]
    let valid: bool = is_valid_utf8(bytes.view())
    let position: usize = find("hello", "ell") otherwise { return 1 }
    let found: bool = contains("hello", "ell") && starts_with("hello", "he") && ends_with("hello", "lo")
    var parts: Vec<String> = split("a::b", "::") catch error { return 2 }
    var values = Vec [1, 2, 3]
    retain(&+values, (value) { value != 2 })
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
        r#"use std/io.{File, Reader, Writer, open_path}
use std/io_buffer.{BufReader, BufWriter}
use std/num.{i32_to_string, parse_i32, parse_u8, parse_usize, usize_to_string}
use std/path.Utf8Path
use std/process.{arg, arg_count, environment, environment_count}
use std/string.bytes
use std/vec.Vec

func main(): i32 {
    let path = Utf8Path.new("file.txt") catch error { return 1 }
    let child = path.join("child") catch error { return 2 }
    let count: usize = parse_usize("42") otherwise { return 3 }
    let signed: i32 = parse_i32("-7") otherwise { return 4 }
    let byte: u8 = parse_u8("8") otherwise { return 5 }
    let first_arg: &str = arg(0) catch error { return 6 } otherwise { return 7 }
    let process_arg_count: usize = arg_count()
    let process_environment_count: usize = environment_count()
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_text_and_collection_operations_run() {
    let project = TempProject::new("distributed-home-practical-std-run");
    let source = project.write_source(
        "practical_std_run.nct",
r#"use std/string.{bytes, find, is_valid_utf8, split}
use std/vec.Vec

func rejects_invalid_utf8(candidate: &[u8]): bool {
    let accepted = String.from_utf8(candidate) catch error {
        return true
    }
    return false
}

func rejects_empty_separator(): bool {
    let accepted: Vec<String> = split("abc", "") catch error {
        return true
    }
    return false
}

func main(): i32 {
    if bytes("hello").len() != 5 || bytes("e").len() != 1 { return 19 }
    let left: u8 = bytes("hello")[1]
    let right: u8 = bytes("e")[0]
    if left != right { return 18 }
    let position: usize = find("hello", "e") otherwise { return 20 }
    if position != 1 { return 21 }
    if !String "hello".contains("ell") { return 2 }
    if !String "hello".starts_with("he") || !String "hello".ends_with("lo") { return 3 }
    let invalid: Vec<u8> = Vec [240, 40, 140, 40]
    if is_valid_utf8(invalid.view()) { return 4 }
    if !rejects_invalid_utf8(invalid.view()) { return 4 }
    let encoded: Vec<u8> = Vec [104, 195, 169]
    let decoded = String.from_utf8(encoded.view()) catch error { return 5 }
    if decoded.view() != "hé" { return 6 }

    var parts = split("a::b::", "::") catch error { return 7 }
    if parts.len() != 3 { return 8 }
    let final_part = parts.pop() otherwise { return 9 }
    let middle_part = parts.pop() otherwise { return 10 }
    let first_part = parts.pop() otherwise { return 11 }
    if first_part.view() != "a" || middle_part.view() != "b" || final_part.view() != "" { return 12 }
    if !rejects_empty_separator() { return 12 }

    var values = Vec [1, 2, 3, 4, 5]
    values.retain((value) { value % 2 != 0 })
    if values.len() != 3 || values.view()[0] != 1 || values.view()[1] != 3 || values.view()[2] != 5 { return 13 }
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
    let source_text = r#"use std/io.{append_path, create_path, open_path, stdout}
use std/io_buffer.{BufReader, BufWriter}
use std/num.{parse_i32, usize_to_string}
use std/path.Utf8Path
use std/process.{arg, arg_count}
use std/string.bytes
use std/vec.Vec

func main(): i32 {
    let path = Utf8Path.new("__PATH__") catch error { return 1 }
    if !path.is_absolute() { return 2 }
    let file = open_path(&path) catch error { return 3 }
    var reader = BufReader.with_capacity(move file, 3)
    var buffer: Vec<u8> = Vec [0, 0, 0, 0, 0, 0]
    let received: usize = reader.read(buffer.view_mut()) catch error { return 4 }
    if received != 6 || buffer.view()[0] != 97 || buffer.view()[5] != 10 { return 5 }

    let created_path = Utf8Path.new("__OUTPUT__") catch error { return 14 }
    let created_file = create_path(&created_path) catch error { return 15 }
    var file_writer = BufWriter.with_capacity(move created_file, 2)
    file_writer.write(bytes("written")) catch error { return 16 }
    file_writer.close() catch error { return 17 }
    let reopened = open_path(&created_path) catch error { return 18 }
    var verifier = BufReader.with_capacity(move reopened, 2)
    var verification: Vec<u8> = Vec [0, 0, 0, 0, 0, 0, 0]
    let verified: usize = verifier.read(verification.view_mut()) catch error { return 19 }
    if verified != 7 || verification.view()[0] != 119 || verification.view()[6] != 110 { return 20 }
    let appended_file = append_path(&created_path) catch error { return 21 }
    var appender = BufWriter.with_capacity(move appended_file, 1)
    appender.write(bytes("!")) catch error { return 22 }
    appender.close() catch error { return 23 }

    let number: i32 = parse_i32("-2147483648") otherwise { return 6 }
    if number != -2147483648 || usize_to_string(42).view() != "42" { return 7 }
    if arg_count() == 0 { return 8 }
    let executable: &str = arg(0) catch error { return 9 } otherwise { return 10 }
    if executable.len() == 0 { return 11 }

    var writer = BufWriter.with_capacity(stdout(), 2)
    writer.write(bytes("ok")) catch error { return 12 }
    writer.flush() catch error { return 13 }
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
