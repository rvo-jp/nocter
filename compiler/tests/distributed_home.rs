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
        r#"from std/fmt import append_bool, append_i32, append_str, append_string, unsupported as fmt_unsupported
from std/io import File, from_os_error, print, stderr, stdout, unsupported as io_unsupported, write_text
from std/mem import Allocator, Layout, RawBuffer, alloc, free, invalid_argument, out_of_memory, page_allocator
from std/os import OSError, OSErrorKind, Platform
from std/process import abort, args, cwd, env, exit
from std/ptr import addr, from_ref, from_ref_mut
from std/string import capacity_overflow, empty, from_str, push_str, view, with_capacity

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
fn distributed_std_error_helper_return_runs() {
    let project = TempProject::new("distributed-home-error-helper-return-run");
    let source = project.write_source(
        "error_helper_return.nct",
        r#"from std/mem import invalid_argument

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

#[test]
fn distributed_std_string_empty_passes_check() {
    let project = TempProject::new("distributed-home-string-empty");
    let source = project.write_source(
        "string_empty.nct",
        r#"from std/string import empty, view

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
fn distributed_std_string_associated_api_passes_check() {
    let project = TempProject::new("distributed-home-string-associated-api");
    let source = project.write_source(
        "string_associated_api.nct",
        r#"from std/mem import page_allocator

func main(): i32! {
    var allocator = page_allocator()
    var empty = String.empty()
    String.push_str(&+empty, "Grow")?
    let empty_view = String.view(&empty)
    var text = String.with_capacity(&+allocator, 16)?
    String.push_str(&+text, empty_view)?
    let copy = String.from_str(&+allocator, String.view(&text))?
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
        r#"from std/string import String

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
        r#"from std/io import File, stdout

func main(): i32! {
    let input = File.open("input.txt") catch error {
        return 0
    }
    var out = stdout()
    out.write_text("checked")?
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_io_file_representation_is_private() {
    let project = TempProject::new("distributed-home-io-file-private");
    let source = project.write_source(
        "io_file_private.nct",
        r#"from std/io import File

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
fn distributed_io_raw_helper_is_not_public_api() {
    let project = TempProject::new("distributed-home-io-raw-private");
    let source = project.write_source(
        "io_raw_private.nct",
        r#"from std/io import write_text_raw

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
fn distributed_std_abort_builds_to_macho() {
    let project = TempProject::new("distributed-home-abort-build");
    let source = project.write_source(
        "abort_app.nct",
        r#"from std/process import abort

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

#[test]
fn distributed_std_explicit_string_construction_builds_to_macho() {
    let project = TempProject::new("distributed-home-explicit-string-build");
    let source = project.write_source(
        "explicit_string_app.nct",
        r#"from std/fmt import append_str
from std/mem import page_allocator
from std/string import with_capacity

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
        r#"from std/fmt import append_str
from std/io import print
from std/mem import page_allocator
from std/string import view, with_capacity

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
        r#"from std/io import print

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
        r#"from std/io import print
from std/mem import page_allocator
from std/string import from_str, view

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
fn distributed_std_string_copy_view_runs() {
    let project = TempProject::new("distributed-home-string-copy-view-run");
    let source = project.write_source(
        "string_copy_view.nct",
        r#"from std/io import print
from std/mem import page_allocator
from std/string import view

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
fn distributed_std_string_associated_api_runs() {
    let project = TempProject::new("distributed-home-string-associated-api-run");
    let source = project.write_source(
        "string_associated_api_run.nct",
        r#"from std/io import print
from std/mem import page_allocator

func main(): i32! {
    var allocator = page_allocator()
    var text = String.with_capacity(&+allocator, 32)?
    String.push_str(&+text, "Hello")?
    let suffix = String.from_str(&+allocator, " Associated")?
    String.push_str(&+text, String.view(&suffix))?
    print(String.view(&text))?
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
fn distributed_std_string_push_str_runs() {
    let project = TempProject::new("distributed-home-string-push-str-run");
    let source = project.write_source(
        "string_push_str.nct",
        r#"from std/io import print
from std/mem import page_allocator
from std/string import push_str, view, with_capacity

func main(): i32! {
    var allocator = page_allocator()
    var text = with_capacity(&+allocator, 5)?
    push_str(&+text, "Hello")?
    push_str(&+text, " String")?
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
fn distributed_std_string_empty_push_str_runs() {
    let project = TempProject::new("distributed-home-string-empty-push-str-run");
    let source = project.write_source(
        "string_empty_push_str.nct",
        r#"from std/io import print
from std/string import empty, push_str, view

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
        r#"from std/fmt import append_str
from std/io import print
from std/mem import page_allocator
from std/string import view, with_capacity

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
        r#"from std/fmt import append_bool, append_str, append_string
from std/io import print
from std/mem import page_allocator
from std/string import from_str, view, with_capacity

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
        r#"from std/fmt import append_i32, append_str
from std/io import print
from std/mem import page_allocator
from std/string import view, with_capacity

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
fn distributed_std_file_write_text_runs() {
    let project = TempProject::new("distributed-home-file-write-text-run");
    let source = project.write_source(
        "file_write_text.nct",
        r#"from std/io import stdout, write_text

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
    fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
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
