use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn distributed_nocter_home_passes_doctor() {
    let output = Command::new(NOCTER)
        .arg("doctor")
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        text(&output.stdout).contains("ok"),
        "doctor stdout should contain ok:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn installed_nocter_uses_executable_parent_as_home_without_env() {
    let install = TempProject::new("distributed-home-installed-layout");
    let home = install.root();
    fs::write(
        home.join("VERSION"),
        fs::read_to_string(distributed_home().join("VERSION")).unwrap(),
    )
    .unwrap();
    fs::write(
        home.join("MANIFEST.json"),
        fs::read_to_string(distributed_home().join("MANIFEST.json")).unwrap(),
    )
    .unwrap();
    fs::create_dir_all(home.join("std")).unwrap();

    let installed = home.join("nocter");
    fs::copy(NOCTER, &installed).unwrap();
    fs::set_permissions(&installed, fs::metadata(NOCTER).unwrap().permissions()).unwrap();

    let output = Command::new(&installed)
        .arg("doctor")
        .env_remove("NOCTER_HOME")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    let stdout = text(&output.stdout);
    let expected_home = home.canonicalize().unwrap();
    assert!(
        stdout.contains(&format!("Nocter home: {}", expected_home.display()))
            && stdout.contains("ok"),
        "doctor stdout should report executable-parent home and ok:\n{stdout}"
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn installed_nocter_lsp_uses_executable_parent_as_home_without_env() {
    let install = TempProject::new("distributed-home-lsp-installed-layout");
    let home = install.root().join(".nocter");
    write_minimal_nocter_home(&home);

    let installed = home.join("nocter");
    fs::copy(NOCTER, &installed).unwrap();
    fs::set_permissions(&installed, fs::metadata(NOCTER).unwrap().permissions()).unwrap();

    let workspace = install.root().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let source = workspace.join("app.nct");
    let source_text = "func main(): i32 {\n    return 0\n}\n";
    fs::write(&source, source_text).unwrap();
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &installed,
        &workspace,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );

    let diagnostics = read_frames(&output.stdout)
        .into_iter()
        .find(|message| message["method"] == "textDocument/publishDiagnostics")
        .and_then(|message| message["params"]["diagnostics"].as_array().cloned())
        .expect("expected diagnostics notification");
    assert!(
        diagnostics.is_empty(),
        "installed LSP should resolve std/prelude without NOCTER_HOME, got:\n{diagnostics:#?}"
    );
}

#[test]
fn distributed_std_public_api_passes_check() {
    let project = TempProject::new("distributed-home-smoke");
    let source = project.write_source(
        "std_smoke.nct",
        r#"use std/fmt.{append_bool, append_i32, append_str, append_string, append_usize}
use std/io.{File, open, print, read, stderr, stdout, write, write_text}
use std/mem.{Allocator, Layout, RawBuffer, alloc, bytes as raw_bytes, bytes_mut as raw_bytes_mut, free, invalid_argument, out_of_memory, page_allocator, prefix as raw_prefix, prefix_mut as raw_prefix_mut}
use std/process.{abort, args, cwd, env, exit}
use std/ptr.{addr, from_ref, from_ref_mut}
use std/string.{bytes, capacity, capacity_overflow, clear, empty, from_str, is_empty, len, push_str, reserve, view, with_capacity}
use std/vec.Vec

func main(): i32 {
    return 0
}

func raw_buffer_view(buffer: &RawBuffer): &[u8] {
    return raw_bytes(buffer)
}

func raw_buffer_view_mut(buffer: &+RawBuffer): &+[u8] {
    return raw_bytes_mut(buffer)
}

func raw_buffer_prefix(buffer: &RawBuffer, prefix_len: usize): &[u8]! {
    return raw_prefix(buffer, prefix_len)?
}

func raw_buffer_prefix_mut(buffer: &+RawBuffer, prefix_len: usize): &+[u8]! {
    return raw_prefix_mut(buffer, prefix_len)?
}

func string_len(text: &String): usize {
    return len(text)
}

func string_capacity(text: &String): usize {
    return capacity(text)
}

func string_is_empty(text: &String): bool {
    return is_empty(text)
}

func string_reserve(text: &+String, additional: usize): void! {
    reserve(text, additional)?
    return
}

func string_clear(text: &+String): void {
    clear(text)
    return
}

func file_read(file: &+File, buffer: &+[u8]): usize! {
    return read(file, buffer)?
}

func file_open(path: &str): File! {
    return open(path)?
}

func file_write(file: &+File, buffer: &[u8]): void! {
    write(file, buffer)?
    return
}

func file_write_text(file: &+File, text: &str): void! {
    write_text(file, text)?
    return
}

func process_cwd(allocator: &+Allocator): String! {
    return cwd(allocator)?
}

func process_args_shape(): Vec<&str>! {
    return args()?
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_prelude_exports_v0_core_names() {
    let project = TempProject::new("distributed-home-prelude-core");
    let source = project.write_source(
        "prelude_core.nct",
        r#"func code(): ErrorCode {
    return "app.ok"
}

func make(): String {
    return String.empty()
}

func main(): i32 {
    let text = make()
    let label = code()
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_prelude_does_not_export_int() {
    let project = TempProject::new("distributed-home-prelude-no-int");
    let source = project.write_source(
        "prelude_no_int.nct",
        r#"func main(): i32 {
    let count: Int = 1
    return count
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("Int") && stderr.contains("not declared"),
        "expected unresolved Int diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_std_vec_requires_explicit_import() {
    let project = TempProject::new("distributed-home-vec-explicit-import");
    let source = project.write_source(
        "vec_explicit_import.nct",
        r#"use std/vec.Vec

func len_placeholder(values: &Vec<&str>): i32 {
    return 0
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_vec_contract_shape_passes_check() {
    let project = TempProject::new("distributed-home-vec-contract-shape");
    let source = project.write_source(
        "vec_contract_shape.nct",
        r#"use std/mem.Allocator
use std/vec.{Vec, capacity, clear, from_slice, is_empty, len, push, reserve, view, view_mut}

func inspect(values: &Vec<usize>): usize {
    return len(values) + capacity(values) + view(values).len()
}

func empty_check(values: &Vec<usize>): bool {
    return is_empty(values)
}

func mutate(values: &+Vec<usize>, value: usize): usize! {
    reserve(values, 0)?
    push(values, value)?
    clear(values)
    return view_mut(values).len()
}

func method_shape(allocator: &+Allocator, values: &[usize]): usize! {
    var owned = from_slice(allocator, values)?
    owned.reserve(0)?
    owned.push(1)?
    owned.clear()
    if owned.is_empty() {
        return 0
    }
    return owned.len() + owned.capacity() + owned.view().len() + owned.view_mut().len()
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_with_capacity_zero_runs() {
    let project = TempProject::new("distributed-home-vec-with-capacity-zero-run");
    let source = project.write_source(
        "vec_with_capacity_zero.nct",
        r#"use std/mem.page_allocator
use std/vec.{Vec, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    let values: Vec<u8> = with_capacity(&+allocator, 0)?
    if values.is_empty() {
        return 42
    }
    return 1
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
fn distributed_std_vec_empty_shape_runs() {
    let project = TempProject::new("distributed-home-vec-empty-shape-run");
    let source = project.write_source(
        "vec_empty_shape.nct",
        r#"use std/vec.{Vec, empty}

func main(): i32 {
    let first: Vec<u8> = Vec.empty()
    if !first.is_empty() {
        return 1
    }
    if first.len() != 0 {
        return 2
    }
    if first.capacity() != 0 {
        return 3
    }
    if first.view().len() != 0 {
        return 4
    }

    var second: Vec<u8> = empty()
    if second.view_mut().len() != 0 {
        return 5
    }
    second.clear()
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_empty_drop_runs() {
    let project = TempProject::new("distributed-home-vec-empty-drop-run");
    let source = project.write_source(
        "vec_empty_drop.nct",
        r#"use std/vec.Vec

func main(): i32 {
    var values: Vec<u8> = Vec.empty()
    drop values

    let scoped: Vec<u8> = Vec.empty()
    if scoped.is_empty() {
        return 0
    }
    return 1
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_nonzero_capacity_reports_unsupported() {
    let project = TempProject::new("distributed-home-vec-nonzero-capacity-unsupported");
    let source = project.write_source(
        "vec_nonzero_capacity_unsupported.nct",
        r#"use std/mem.page_allocator
use std/vec.{Vec, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    let values: Vec<u8> = with_capacity(&+allocator, 1)?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert_eq!(
        output.stderr,
        b"std.vec.unsupported: Vec storage is not implemented\n"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_push_reports_unsupported() {
    let project = TempProject::new("distributed-home-vec-push-unsupported");
    let source = project.write_source(
        "vec_push_unsupported.nct",
        r#"use std/vec.Vec

func main(): i32! {
    var values: Vec<u8> = Vec.empty()
    values.push(1)?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert_eq!(
        output.stderr,
        b"std.vec.unsupported: Vec storage is not implemented\n"
    );
}

#[test]
fn distributed_std_vec_fields_are_private() {
    let project = TempProject::new("distributed-home-vec-fields-private");
    let source = project.write_source(
        "vec_fields_private.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec<usize>{
        len: 0,
    }
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("hidden field") || stderr.contains("not visible"),
        "expected private Vec field diagnostic, got:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_error_helper_return_runs() {
    let project = TempProject::new("distributed-home-error-helper-return-run");
    let source = project.write_source(
        "error_helper_return.nct",
        r#"use std/mem.invalid_argument

func main(): void! {
    return invalid_argument()
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert_eq!(
        output.stderr,
        b"std.mem.invalid_argument: invalid allocation request\n"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_os_error_model_is_not_public_api() {
    let project = TempProject::new("distributed-home-os-error-private");
    let source = project.write_source(
        "os_error_private.nct",
        r#"use std/os.OSErrorKind

func main(): i32 {
    let kind = OSErrorKind.not_found
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "expected empty stdout");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_std_string_empty_passes_check() {
    let project = TempProject::new("distributed-home-string-empty");
    let source = project.write_source(
        "string_empty.nct",
        r#"use std/string.{empty, view}

func main(): i32 {
    let text = empty()
    let slice = view(&text)
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_string_method_api_passes_check() {
    let project = TempProject::new("distributed-home-string-method-api");
    let source = project.write_source(
        "string_method_api.nct",
        r#"use std/mem.page_allocator

func main(): i32! {
    var allocator = page_allocator()
    var empty = String.empty()
    if !empty.is_empty() {
        return 1
    }
    empty.push_str("Grow")?
    if empty.len() != 4 {
        return 2
    }
    let empty_view = empty.view()
    var text = String.with_capacity(&+allocator, 16)?
    if text.capacity() != 16 {
        return 3
    }
    text.push_str(empty_view)?
    let copy = String.from_str(&+allocator, text.view())?
    if copy.len() != 4 {
        return 4
    }
    empty.reserve(8)?
    empty.clear()
    if !empty.is_empty() {
        return 5
    }
    drop copy
    drop text
    drop empty
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_string_representation_is_private() {
    let project = TempProject::new("distributed-home-string-private");
    let source = project.write_source(
        "string_private.nct",
        r#"use std/string.String

func main(): i32 {
    let text = String{ len: 0 }
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0377") && stderr.contains("not visible here"),
        "expected private String field diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_file_methods_pass_check() {
    let project = TempProject::new("distributed-home-io-file-methods");
    let source = project.write_source(
        "io_file_methods.nct",
        r#"use std/io.{File, open, stdout}

func main(): i32! {
    let input = File.open("input.txt") catch error {
        return 0
    }
    let opened = open("input.txt") catch error {
        return 0
    }
    var out = stdout()
    out.write_text("checked")?
    return 0
}

func read_into(buffer: &+[u8]): usize! {
    var input = File.open("input.txt")?
    return input.read(buffer)?
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_io_file_open_runs() {
    let project = TempProject::new("distributed-home-io-file-open-run");
    fs::write(project.root().join("input.txt"), b"open me").unwrap();
    let source = project.write_source(
        "file_open.nct",
        r#"use std/io.File

func main(): i32! {
    var input = File.open("input.txt")?
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_io_file_open_missing_reports_error() {
    let project = TempProject::new("distributed-home-io-file-open-missing");
    let source = project.write_source(
        "file_open_missing.nct",
        r#"use std/io.File

func main(): i32! {
    var input = File.open("missing.txt")?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "expected empty stdout");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("std.io.not_found") && stderr.contains("file not found"),
        "stderr:\n{}",
        stderr
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_io_file_read_raw_buffer_runs() {
    let project = TempProject::new("distributed-home-io-file-read-raw-buffer-run");
    fs::write(project.root().join("input.txt"), b"Hi").unwrap();
    let source = project.write_source(
        "file_read_raw_buffer.nct",
        r#"use std/io.{File, stdout}
use std/mem.{RawBuffer, alloc, free, page_allocator}

func main(): i32! {
    var allocator = page_allocator()
    var buffer = alloc(&+allocator, 4, 1)?
    var input = File.open("input.txt")?
    let count: usize = input.read(buffer.bytes_mut())?
    var out = stdout()
    let bytes: &[u8] = buffer.prefix(count)?
    out.write(bytes)?
    free(&+allocator, move buffer)
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
    assert_eq!(output.stdout, b"Hi");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_io_top_level_read_write_runs() {
    let project = TempProject::new("distributed-home-io-top-level-read-write-run");
    fs::write(project.root().join("input.txt"), b"IO").unwrap();
    let source = project.write_source(
        "io_top_level_read_write.nct",
        r#"use std/io.{open, stdout, read, write}
use std/mem.{alloc, free, page_allocator}

func main(): i32! {
    var allocator = page_allocator()
    var buffer = alloc(&+allocator, 4, 1)?
    var input = open("input.txt")?
    let count: usize = read(&+input, buffer.bytes_mut())?
    var out = stdout()
    let bytes: &[u8] = buffer.prefix(count)?
    write(&+out, bytes)?
    free(&+allocator, move buffer)
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
    assert_eq!(output.stdout, b"IO");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[test]
fn distributed_io_file_write_bytes_builds_to_macho() {
    let project = TempProject::new("distributed-home-file-write-bytes-build");
    let source = project.write_source(
        "file_write_bytes.nct",
        r#"use std/io.stdout

func write_bytes(bytes: &[u8]): void! {
    var out = stdout()
    out.write(bytes)?
    return
}

func main(): i32 {
    return 0
}
"#,
    );
    let executable = project.root().join("file_write_bytes");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_io_file_write_string_bytes_runs() {
    let project = TempProject::new("distributed-home-file-write-string-bytes-run");
    let source = project.write_source(
        "file_write_string_bytes.nct",
        r#"use std/io.stdout
use std/string.bytes

func main(): i32! {
    var out = stdout()
    out.write(bytes("Hello bytes"))?
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
    assert_eq!(output.stdout, b"Hello bytes");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn distributed_io_file_representation_is_private() {
    let project = TempProject::new("distributed-home-io-file-private");
    let source = project.write_source(
        "io_file_private.nct",
        r#"use std/io.File

func main(): i32 {
    let file = File{ close_on_drop: false }
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0377") && stderr.contains("not visible here"),
        "expected private File field diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_os_error_converter_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-os-error-converter-private");
    let source = project.write_source(
        "io_os_error_converter_private.nct",
        r#"use std/io.from_os_error

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_fmt_unsupported_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-fmt-unsupported-private");
    let source = project.write_source(
        "fmt_unsupported_private.nct",
        r#"use std/fmt.unsupported

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("cannot access private name `unsupported`"),
        "expected private fmt unsupported diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_unsupported_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-unsupported-private");
    let source = project.write_source(
        "io_unsupported_private.nct",
        r#"use std/io.unsupported

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("cannot access private name `unsupported`"),
        "expected private io unsupported diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-raw-private");
    let source = project.write_source(
        "io_raw_private.nct",
        r#"use std/io.write_text_raw

func main(): i32 {
    write_text_raw(1, "x")!
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_byte_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-byte-raw-private");
    let source = project.write_source(
        "io_byte_raw_private.nct",
        r#"use std/io.write_bytes_raw

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_read_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-read-raw-private");
    let source = project.write_source(
        "io_read_raw_private.nct",
        r#"use std/io.read_bytes_raw

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_ptr_store_u8_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-ptr-store-u8-private");
    let source = project.write_source(
        "ptr_store_u8_private.nct",
        r#"use std/ptr.store_u8_to_ptr

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_ptr_slice_raw_parts_helpers_are_not_public_api() {
    let project = TempProject::new("distributed-home-ptr-slice-raw-parts-private");
    let source = project.write_source(
        "ptr_slice_raw_parts_private.nct",
        r#"use std/ptr.slice_from_raw_parts
use std/ptr.slice_from_raw_parts_mut

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_open_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-open-raw-private");
    let source = project.write_source(
        "io_open_raw_private.nct",
        r#"use std/io.open_read_raw

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_io_close_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-close-raw-private");
    let source = project.write_source(
        "io_close_raw_private.nct",
        r#"use std/io.close_fd_raw

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_process_exit_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-process-exit-raw-private");
    let source = project.write_source(
        "process_exit_raw_private.nct",
        r#"use std/process.exit_raw

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0412") && stderr.contains("pub(nocter)"),
        "expected pub(nocter) visibility diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_std_abort_builds_to_macho() {
    let project = TempProject::new("distributed-home-abort-build");
    let source = project.write_source(
        "abort_app.nct",
        r#"use std/process.abort

func main(): i32 {
    abort()
}
"#,
    );
    let executable = project.root().join("abort_app");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_process_exit_runs_with_requested_code() {
    let project = TempProject::new("distributed-home-process-exit-run");
    let source = project.write_source(
        "process_exit_app.nct",
        r#"use std/process.exit

func main(): i32 {
    return exit(7)
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_process_env_shape_passes_check() {
    let project = TempProject::new("distributed-home-process-env-check");
    let source = project.write_source(
        "process_env_shape.nct",
        r#"use std/process.env

func lookup(): &str?! {
    return env("HOME")?
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_process_args_builds_to_macho() {
    let project = TempProject::new("distributed-home-process-args-build");
    let source = project.write_source(
        "process_args_build.nct",
        r#"use std/process.args

func main(): i32! {
    let values = args()?
    return 0
}
"#,
    );
    let executable = project.root().join("process_args_check_only");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_process_args_reports_runtime_unsupported() {
    let project = TempProject::new("distributed-home-process-args-runtime-unsupported");
    let source = project.write_source(
        "process_args_runtime_unsupported.nct",
        r#"use std/process.args

func main(): i32! {
    let values = args()?
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"std.process.unsupported: process arguments are not implemented\n"
    );
}

#[test]
fn distributed_std_process_env_is_check_only_for_build() {
    let project = TempProject::new("distributed-home-process-env-check-only-build");
    let source = project.write_source(
        "process_env_check_only.nct",
        r#"use std/process.env as lookup

func main(): i32! {
    let value = lookup("HOME")?
    return 0
}
"#,
    );
    let executable = project.root().join("process_env_check_only");

    let output = nocter_build(&project, &source, &executable);

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("check-only `std/process.env` calls"),
        "expected env check-only diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("4 |     let value = lookup(\"HOME\")?"),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("nested fallible or optional return types"),
        "std internal return-shape diagnostic should not leak for check-only calls, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("error[E800"),
        "buildability preflight should reject before IR lowering, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after preflight diagnostics"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_process_cwd_returns_current_directory() {
    let project = TempProject::new("distributed-home-process-cwd-run");
    let source = project.write_source(
        "process_cwd.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/process.cwd

func main(): void! {
    var allocator = page_allocator()
    let value = cwd(&+allocator)?
    print(value.view())?
    return
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
    let expected = project.root().canonicalize().unwrap();
    assert_eq!(text(&output.stdout), expected.display().to_string());
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[test]
fn distributed_std_explicit_string_construction_builds_to_macho() {
    let project = TempProject::new("distributed-home-explicit-string-build");
    let source = project.write_source(
        "explicit_string_app.nct",
        r#"use std/fmt.append_str
use std/mem.page_allocator
use std/string.with_capacity

func make(): String! {
    var allocator = page_allocator()
    var out = with_capacity(&+allocator, 8)?
    append_str(&+out, "hello")?
    return move out
}

func main(): i32! {
    var text = make()?
    return 0
}
"#,
    );
    let executable = project.root().join("explicit_string_app");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_explicit_string_construction_runs() {
    let project = TempProject::new("distributed-home-explicit-string-run");
    let source = project.write_source(
        "explicit_string_run.nct",
        r#"use std/fmt.append_str
use std/io.print
use std/mem.page_allocator
use std/string.{view, with_capacity}

func make(): String! {
    var allocator = page_allocator()
    var out = with_capacity(&+allocator, 8)?
    append_str(&+out, "hello")?
    append_str(&+out, " runtime")?
    return move out
}

func main(): i32! {
    var text = make()?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"hello runtime");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_print_hello_runs() {
    let project = TempProject::new("distributed-home-print-hello-run");
    let source = project.write_source(
        "hello.nct",
        r#"use std/io.print

func main(): i32! {
    print("Hello")?
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
    assert_eq!(output.stdout, b"Hello");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_from_str_view_runs() {
    let project = TempProject::new("distributed-home-string-from-str-view-run");
    let source = project.write_source(
        "string_from_str_view.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/string.{from_str, view}

func main(): i32! {
    var allocator = page_allocator()
    let text = from_str(&+allocator, "Hello String")?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"Hello String");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_bytes_file_write_runs() {
    let project = TempProject::new("distributed-home-string-bytes-file-write-run");
    let source = project.write_source(
        "string_bytes_file_write.nct",
        r#"use std/io.stdout
use std/mem.page_allocator
use std/string.from_str

func main(): i32! {
    var allocator = page_allocator()
    let text = from_str(&+allocator, "Owned bytes")?
    var out = stdout()
    out.write(text.bytes())?
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
    assert_eq!(output.stdout, b"Owned bytes");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_from_str_forward_return_runs() {
    let project = TempProject::new("distributed-home-string-from-str-forward-return-run");
    let source = project.write_source(
        "string_from_str_forward_return.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/string.{from_str, view}

func make(): String! {
    var allocator = page_allocator()
    return from_str(&+allocator, "Forwarded String")?
}

func main(): i32! {
    var text = make()?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"Forwarded String");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_copy_view_runs() {
    let project = TempProject::new("distributed-home-string-copy-view-run");
    let source = project.write_source(
        "string_copy_view.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/string.view

func main(): i32! {
    var allocator = page_allocator()
    let text = String.copy(&+allocator, "Copied String")?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"Copied String");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_method_api_runs() {
    let project = TempProject::new("distributed-home-string-method-api-run");
    let source = project.write_source(
        "string_method_api_run.nct",
        r#"use std/io.print
use std/mem.page_allocator

func main(): i32! {
    var allocator = page_allocator()
    var text = String.with_capacity(&+allocator, 32)?
    text.push_str("Hello")?
    let suffix = String.from_str(&+allocator, " Associated")?
    text.push_str(suffix.view())?
    print(text.view())?
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
    assert_eq!(output.stdout, b"Hello Associated");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_view_equality_runs() {
    let project = TempProject::new("distributed-home-string-view-equality-run");
    let source = project.write_source(
        "string_view_equality.nct",
        r#"use std/mem.page_allocator

func main(): i32! {
    var allocator = page_allocator()
    let text = String.from_str(&+allocator, "Nocter")?
    if text.view() == "Nocter" && text.view() != "Other" {
        return 42
    } else {
        return 1
    }
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
fn distributed_std_string_push_str_runs() {
    let project = TempProject::new("distributed-home-string-push-str-run");
    let source = project.write_source(
        "string_push_str.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/string.{capacity, clear, is_empty, len, push_str, reserve, view, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 5)?
    if !is_empty(&text) {
        return 1
    }
    reserve(&+text, 5)?
    if capacity(&text) != 5 {
        return 2
    }
    push_str(&+text, "Hello")?
    if len(&text) != 5 {
        return 3
    }
    reserve(&+text, 7)?
    push_str(&+text, " String")?
    if len(&text) != 12 {
        return 4
    }
    if capacity(&text) != 12 {
        return 5
    }
    print(view(&text))?
    clear(&+text)
    if !is_empty(&text) {
        return 6
    }
    if capacity(&text) != 12 {
        return 7
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
    assert_eq!(output.stdout, b"Hello String");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_string_empty_push_str_runs() {
    let project = TempProject::new("distributed-home-string-empty-push-str-run");
    let source = project.write_source(
        "string_empty_push_str.nct",
        r#"use std/io.print
use std/string.{empty, push_str, view}

func main(): i32! {
    var text = empty()
    push_str(&+text, "Grow")?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"Grow");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_fmt_append_str_runs() {
    let project = TempProject::new("distributed-home-fmt-append-str-run");
    let source = project.write_source(
        "fmt_append_str.nct",
        r#"use std/fmt.append_str
use std/io.print
use std/mem.page_allocator
use std/string.{view, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 16)?
    append_str(&+text, "Hello")?
    append_str(&+text, " Format")?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"Hello Format");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_fmt_append_bool_and_string_runs() {
    let project = TempProject::new("distributed-home-fmt-append-bool-string-run");
    let source = project.write_source(
        "fmt_append_bool_string.nct",
        r#"use std/fmt.{append_bool, append_str, append_string}
use std/io.print
use std/mem.page_allocator
use std/string.{from_str, view, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 32)?
    append_bool(&+text, true)?
    append_str(&+text, " ")?
    append_bool(&+text, false)?
    let suffix = from_str(&+allocator, " done")?
    append_string(&+text, &suffix)?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"true false done");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_fmt_append_i32_runs() {
    let project = TempProject::new("distributed-home-fmt-append-i32-run");
    let source = project.write_source(
        "fmt_append_i32.nct",
        r#"use std/fmt.{append_i32, append_str}
use std/io.print
use std/mem.page_allocator
use std/string.{view, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 4)?
    append_i32(&+text, 0)?
    append_str(&+text, " ")?
    append_i32(&+text, 42)?
    append_str(&+text, " ")?
    append_i32(&+text, -17)?
    append_str(&+text, " ")?
    append_i32(&+text, -2147483648)?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"0 42 -17 -2147483648");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_fmt_append_usize_runs() {
    let project = TempProject::new("distributed-home-fmt-append-usize-run");
    let source = project.write_source(
        "fmt_append_usize.nct",
        r#"use std/fmt.{append_str, append_usize}
use std/io.print
use std/mem.page_allocator
use std/string.{view, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 8)?
    append_usize(&+text, 0)?
    append_str(&+text, " ")?
    append_usize(&+text, 42)?
    append_str(&+text, " ")?
    append_usize(&+text, 18446744073709551615)?
    print(view(&text))?
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
    assert_eq!(output.stdout, b"0 42 18446744073709551615");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_file_write_text_runs() {
    let project = TempProject::new("distributed-home-file-write-text-run");
    let source = project.write_source(
        "file_write_text.nct",
        r#"use std/io.{stdout, write_text}

func main(): i32! {
    var out = stdout()
    out.write_text("Hello")?
    write_text(&+out, " from File")?
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
    assert_eq!(output.stdout, b"Hello from File");
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_file_write_text_stderr_runs() {
    let project = TempProject::new("distributed-home-file-write-text-stderr-run");
    let source = project.write_source(
        "file_write_text_stderr.nct",
        r#"use std/io.{stderr, write_text}

func main(): i32! {
    var err = stderr()
    err.write_text("Hello")?
    write_text(&+err, " stderr")?
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert_eq!(output.stderr, b"Hello stderr");
}

#[test]
fn distributed_std_sources_do_not_reintroduce_removed_placeholders() {
    let mut files = Vec::new();
    collect_nocter_sources(&distributed_home(), &mut files);

    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("make_error"),
            "`{}` still references removed make_error helper",
            file.display()
        );
        assert!(
            !text.contains(".placeholder("),
            "`{}` still contains parser-only placeholder calls",
            file.display()
        );
        assert!(
            !text.contains("primitive args_impl")
                && !text.contains("primitive env_impl")
                && !text.contains("primitive cwd_impl")
                && !text.contains("primitive error_from_os"),
            "`{}` declares a primitive outside the closed registry",
            file.display()
        );
    }
}

fn nocter_check(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["check", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap()
}

fn nocter_build(project: &TempProject, source: &Path, executable: &Path) -> Output {
    Command::new(NOCTER)
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            executable.to_str().unwrap(),
        ])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap()
}

fn nocter_run(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["run", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap()
}

fn nocter_lsp(nocter: &Path, current_dir: &Path, messages: &[Value]) -> Output {
    let mut child = Command::new(nocter)
        .arg("lsp")
        .current_dir(current_dir)
        .env_remove("NOCTER_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        for message in messages {
            write_frame(stdin, message);
        }
    }
    drop(child.stdin.take());

    child.wait_with_output().unwrap()
}

fn write_frame<W: Write>(writer: &mut W, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    writer.write_all(&body).unwrap();
}

fn read_frames(bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let header_end = find_header_end(&bytes[index..]).expect("expected LSP header") + index;
        let header = std::str::from_utf8(&bytes[index..header_end]).unwrap();
        let content_length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("expected Content-Length header");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        messages.push(serde_json::from_slice(&bytes[body_start..body_end]).unwrap());
        index = body_end;
    }

    messages
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn write_minimal_nocter_home(home: &Path) {
    fs::create_dir_all(home.join("std")).unwrap();
    fs::write(
        home.join("VERSION"),
        fs::read_to_string(distributed_home().join("VERSION")).unwrap(),
    )
    .unwrap();
    fs::write(
        home.join("MANIFEST.json"),
        fs::read_to_string(distributed_home().join("MANIFEST.json")).unwrap(),
    )
    .unwrap();
    fs::write(home.join("std/prelude.nct"), "").unwrap();
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

fn assert_macho_executable(path: &Path) {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() > 4, "expected non-empty Mach-O executable");
    assert_eq!(&bytes[0..4], &[0xcf, 0xfa, 0xed, 0xfe]);
}

fn collect_nocter_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_nocter_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "nct") {
            files.push(path);
        }
    }
}

fn distributed_home() -> PathBuf {
    repo_root().join(".nocter")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live below repository root")
        .to_path_buf()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(name));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
