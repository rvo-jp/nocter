use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
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
fn distributed_release_identity_matches_packaging_metadata() {
    let home = distributed_home();
    let version = fs::read_to_string(home.join("VERSION")).unwrap();
    assert_eq!(version, format!("{}\n", env!("CARGO_PKG_VERSION")));

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(home.join("MANIFEST.json")).unwrap()).unwrap();
    assert_eq!(manifest["release"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["host"], "arm64-darwin");
    assert_eq!(manifest["default_target"], "arm64-darwin");
    assert_eq!(manifest["license"]["id"], "Apache-2.0");
    assert_eq!(manifest["license"]["path"], "LICENSE");
    assert_eq!(manifest["license"]["notice"], "NOTICE");
    assert_eq!(
        manifest["archive"]["name"],
        format!("nocter-v{}-arm64-darwin.tar.gz", env!("CARGO_PKG_VERSION"))
    );

    let license = fs::read_to_string(home.join("LICENSE")).unwrap();
    assert!(license.contains("Apache License"));
    assert!(license.contains("Version 2.0, January 2004"));

    let notice = fs::read_to_string(home.join("NOTICE")).unwrap();
    assert!(notice.contains("Nocter"));
    assert!(notice.contains("Copyright 2026 Rvo JP"));

    let output = Command::new(home.join("nocter"))
        .arg("--version")
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
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );

    let stdout = text(&output.stdout);
    assert!(
        stdout.contains(&format!("Nocter {}", env!("CARGO_PKG_VERSION")))
            && stdout.contains("host: arm64-darwin")
            && stdout.contains("default target: arm64-darwin"),
        "unexpected version output:\n{stdout}"
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
    fs::write(
        home.join("LICENSE"),
        fs::read_to_string(distributed_home().join("LICENSE")).unwrap(),
    )
    .unwrap();
    fs::write(
        home.join("NOTICE"),
        fs::read_to_string(distributed_home().join("NOTICE")).unwrap(),
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

#[cfg(unix)]
#[test]
fn installed_nocter_symlink_uses_target_parent_as_home_without_env() {
    let install = TempProject::new("distributed-home-symlink-installed-layout");
    let home = install.root().join(".nocter");
    write_minimal_nocter_home(&home);

    let installed = home.join("nocter");
    fs::copy(NOCTER, &installed).unwrap();
    fs::set_permissions(&installed, fs::metadata(NOCTER).unwrap().permissions()).unwrap();

    let bin = install.root().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let linked = bin.join("nocter");
    std::os::unix::fs::symlink(&installed, &linked).unwrap();

    let output = Command::new(&linked)
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
        "doctor stdout should report symlink target parent as home and ok:\n{stdout}"
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
use std/mem.{Allocator, Layout, RawBuffer, alloc, alloc_layout, bytes as raw_bytes, bytes_mut as raw_bytes_mut, free, grow, invalid_argument, layout, layout_align, layout_size, out_of_memory, page_allocator, prefix as raw_prefix, prefix_mut as raw_prefix_mut}
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

func allocator_alloc(allocator: &+Allocator, size: usize, align: usize): RawBuffer! {
    return allocator.alloc(size, align)?
}

func allocator_free(allocator: &+Allocator, buffer: RawBuffer): void {
    allocator.free(move buffer)
    return
}

func allocator_layout(): Layout! {
    let value = layout(4, 4)?
    if value.size() != layout_size(&value) {
        return invalid_argument()
    }
    if value.align() != layout_align(&value) {
        return invalid_argument()
    }
    return value
}

func allocator_grow(allocator: &+Allocator, buffer: &+RawBuffer): void! {
    grow(allocator, buffer, 8)?
    allocator.grow(buffer, 16)?
    return
}

func allocator_alloc_layout(allocator: &+Allocator): RawBuffer! {
    return alloc_layout(allocator, Layout.new(8, 8)?)?
}

func allocator_methods(): void! {
    var allocator = page_allocator()
    var buffer = allocator_alloc(&+allocator, 1, 1)?
    allocator_free(&+allocator, move buffer)
    return
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_ptr_from_ref_mut_address_runs() {
    let project = TempProject::new("distributed-home-ptr-from-ref-mut-address-run");
    let source = project.write_source(
        "ptr_from_ref_mut_address.nct",
        r#"use std/ptr.{addr, from_ref_mut}

func main(): i32 {
    var byte: u8 = 1
    let address = address_of(&+byte)
    if address == 0 {
        return 1
    }
    return 0
}

func address_of(value: &+u8): usize {
    return addr(from_ref_mut(value))
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
}

#[test]
fn distributed_std_vec_contract_shape_passes_check() {
    let project = TempProject::new("distributed-home-vec-contract-shape");
    let source = project.write_source(
        "vec_contract_shape.nct",
        r#"use std/mem.Allocator
use std/vec.{Vec, capacity, clear, from_slice, is_empty, len, pop, push, reserve, view, view_mut}

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

func pop_shapes(values: &+Vec<usize>): usize? {
    let free_value = pop(values) otherwise { return none }
    values.push(free_value)!
    return values.pop() otherwise { return none }
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

    let third: Vec<usize> = Vec.empty()
    if third.view().len() != 0 {
        return 6
    }
    if !third.view().is_empty() {
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_from_empty_non_byte_slice_runs() {
    let project = TempProject::new("distributed-home-vec-empty-non-byte-slice-run");
    let source = project.write_source(
        "vec_empty_non_byte_slice.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    let source: Vec<usize> = Vec.empty()
    let view = source.view()
    let copy: Vec<usize> = Vec.from_slice(&+allocator, view)?
    if copy.len() != 0 {
        return 1
    }
    if !copy.view().is_empty() {
        return 2
    }
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_from_scalar_slice_runs() {
    let project = TempProject::new("distributed-home-vec-from-scalar-slice-run");
    let source = project.write_source(
        "vec_from_scalar_slice.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()

    var bytes: Vec<u8> = Vec.empty()
    bytes.push(3)?
    bytes.push(9)?
    let byte_copy: Vec<u8> = Vec.from_slice(&+allocator, bytes.view())?
    if byte_copy.len() != 2 {
        return 1
    }
    if byte_copy.view()[0] != 3 {
        return 2
    }
    if byte_copy.view()[1] != 9 {
        return 3
    }

    var words: Vec<usize> = Vec.empty()
    words.push(13)?
    words.push(21)?
    let word_copy: Vec<usize> = Vec.from_slice(&+allocator, words.view())?
    if word_copy.len() != 2 {
        return 4
    }
    if word_copy.view()[0] != 13 {
        return 5
    }
    if word_copy.view()[1] != 21 {
        return 6
    }

    var numbers: Vec<i32> = Vec.empty()
    numbers.push(34)?
    numbers.push(55)?
    let number_copy: Vec<i32> = Vec.from_slice(&+allocator, numbers.view())?
    if number_copy.len() != 2 {
        return 7
    }
    if number_copy.view()[0] != 34 {
        return 8
    }
    if number_copy.view()[1] != 55 {
        return 9
    }

    var flags: Vec<bool> = Vec.empty()
    flags.push(false)?
    flags.push(true)?
    let flag_copy: Vec<bool> = Vec.from_slice(&+allocator, flags.view())?
    if flag_copy.len() != 2 {
        return 10
    }
    if flag_copy.view()[0] != false {
        return 11
    }
    if flag_copy.view()[1] != true {
        return 12
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_empty_non_byte_slice_index_branch_runs() {
    let project = TempProject::new("distributed-home-vec-empty-non-byte-slice-index-run");
    let source = project.write_source(
        "vec_empty_non_byte_slice_index.nct",
        r#"use std/vec.Vec

func main(): usize {
    let values: Vec<usize> = Vec.empty()
    let view = values.view()
    if view.len() == 0 {
        return 42
    } else {
        return view[0]
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_empty_direct_non_byte_view_index_branch_runs() {
    let project = TempProject::new("distributed-home-vec-empty-direct-non-byte-view-index-run");
    let source = project.write_source(
        "vec_empty_direct_non_byte_view_index.nct",
        r#"use std/vec.Vec

func main(): usize {
    let values: Vec<usize> = Vec.empty()
    if values.view().len() == 0 {
        return 42
    }
    if values.view()[0] == 0 {
        return 1
    }
    return 2
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_member_rooted_slice_index_assignment_runs() {
    let project = TempProject::new("distributed-home-member-rooted-slice-index-assignment-run");
    let source = project.write_source(
        "member_rooted_slice_index_assignment.nct",
        r#"use std/vec.Vec

struct Buffer {
    data: &+[u8]
}

func main(): i32! {
    var values: Vec<u8> = Vec.empty()
    values.push(1)?
    values.push(2)?

    var buffer = Buffer { data: values.view_mut() }
    buffer.data[0] = 9
    buffer.data[1] = 7

    if buffer.data[0] != 9 {
        return 1
    }
    if buffer.data[1] != 7 {
        return 2
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_member_rooted_slice_index_compound_assignment_runs() {
    let project =
        TempProject::new("distributed-home-member-rooted-slice-index-compound-assignment-run");
    let source = project.write_source(
        "member_rooted_slice_index_compound_assignment.nct",
        r#"use std/vec.Vec

struct Buffer {
    data: &+[usize]
}

func main(): i32! {
    var values: Vec<usize> = Vec.empty()
    values.push(10)?
    values.push(20)?

    var buffer = Buffer { data: values.view_mut() }
    buffer.data[0] += 5
    buffer.data[1] *= 2

    if buffer.data[0] != 15 {
        return 1
    }
    if buffer.data[1] != 40 {
        return 2
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
fn distributed_std_vec_nonzero_capacity_runs() {
    let project = TempProject::new("distributed-home-vec-nonzero-capacity-run");
    let source = project.write_source(
        "vec_nonzero_capacity.nct",
        r#"use std/mem.page_allocator
use std/vec.{Vec, with_capacity}

func main(): i32! {
    var allocator = page_allocator()
    let values: Vec<u8> = with_capacity(&+allocator, 1)?
    if values.len() != 0 {
        return 1
    }
    if values.capacity() != 1 {
        return 2
    }
    if !values.is_empty() {
        return 3
    }

    let words: Vec<usize> = Vec.with_capacity(&+allocator, 2)?
    if words.len() != 0 {
        return 4
    }
    if words.capacity() != 2 {
        return 5
    }

    let numbers: Vec<i32> = Vec.with_capacity(&+allocator, 3)?
    if numbers.len() != 0 {
        return 6
    }
    if numbers.capacity() != 3 {
        return 7
    }

    let flags: Vec<bool> = with_capacity(&+allocator, 4)?
    if flags.len() != 0 {
        return 8
    }
    if flags.capacity() != 4 {
        return 9
    }

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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_reserve_empty_runs() {
    let project = TempProject::new("distributed-home-vec-reserve-empty-run");
    let source = project.write_source(
        "vec_reserve_empty.nct",
        r#"use std/vec.{Vec, reserve}

func main(): i32! {
    var values: Vec<u8> = Vec.empty()
    values.reserve(3)?
    if values.len() != 0 {
        return 1
    }
    if values.capacity() != 3 {
        return 2
    }
    if !values.is_empty() {
        return 3
    }
    if values.view().len() != 0 {
        return 4
    }
    reserve(&+values, 1)?
    if values.capacity() != 3 {
        return 5
    }

    var words: Vec<usize> = Vec.empty()
    reserve(&+words, 2)?
    if words.len() != 0 {
        return 6
    }
    if words.capacity() != 2 {
        return 7
    }

    var numbers: Vec<i32> = Vec.empty()
    reserve(&+numbers, 4)?
    if numbers.len() != 0 {
        return 8
    }
    if numbers.capacity() != 4 {
        return 9
    }

    var flags: Vec<bool> = Vec.empty()
    flags.reserve(5)?
    if flags.len() != 0 {
        return 10
    }
    if flags.capacity() != 5 {
        return 11
    }

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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_failed_growth_preserves_elements() {
    let project = TempProject::new("distributed-home-vec-failed-growth-run");
    let source = project.write_source(
        "vec_failed_growth.nct",
        r#"use std/vec.Vec

func grow_huge(values: &+Vec<u8>): void! {
    values.reserve(18446744073709551614)?
    return
}

func preserved(values: &Vec<u8>): i32 {
    if values.len() != 1 {
        return 1
    }
    if values.capacity() != 1 {
        return 2
    }
    if values.view()[0] != 42 {
        return 3
    }
    return 42
}

func main(): i32! {
    var values: Vec<u8> = Vec.empty()
    values.push(42)?
    grow_huge(&+values) catch error {
        return preserved(&values)
    }
    return 4
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
fn distributed_std_vec_string_push_clear_and_drop_runs() {
    let project = TempProject::new("distributed-home-vec-string-ownership-run");
    let source = project.write_source(
        "vec_string_ownership.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<String> = Vec.empty()
    let first = String.from_str(&+allocator, "first")?
    values.push(move first)?
    let second = String.from_str(&+allocator, " second")?
    values.push(move second)?
    values.clear()
    if values.len() != 0 {
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_nested_vec_string_clear_and_drop_runs() {
    let project = TempProject::new("distributed-home-nested-vec-string-ownership-run");
    let source = project.write_source(
        "nested_vec_string_ownership.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    var inner: Vec<String> = Vec.with_capacity(&+allocator, 1)?
    let text = String.from_str(&+allocator, "nested")?
    inner.push(move text)?

    var outer: Vec<Vec<String>> = Vec.with_capacity(&+allocator, 1)?
    outer.push(move inner)?
    outer.clear()
    if outer.len() != 0 {
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_string_pop_transfers_ownership_runs() {
    let project = TempProject::new("distributed-home-vec-string-pop-run");
    let source = project.write_source(
        "vec_string_pop.nct",
        r#"use std/io.print
use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<String> = Vec.with_capacity(&+allocator, 1)?
    let text = String.from_str(&+allocator, "popped")?
    values.push(move text)?
    let popped = values.pop() otherwise { return 2 }
    drop values
    print(popped.view())?
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
    assert_eq!(text(&output.stdout), "popped");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_i32_pop_runs() {
    let project = TempProject::new("distributed-home-vec-i32-pop-run");
    let source = project.write_source(
        "vec_i32_pop.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<i32> = Vec.with_capacity(&+allocator, 1)?
    values.push(42)?
    let popped = values.pop() otherwise { return 2 }
    return popped
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
fn distributed_std_vec_direct_aggregate_pop_runs() {
    let project = TempProject::new("distributed-home-vec-direct-aggregate-pop-run");
    let source = project.write_source(
        "vec_direct_aggregate_pop.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

copy struct Pair {
    value: i32
}

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<Pair> = Vec.with_capacity(&+allocator, 1)?
    values.push(Pair { value: 42 })?
    let popped = values.pop() otherwise { return 2 }
    return popped.value
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
fn distributed_std_vec_empty_pop_returns_none() {
    let project = TempProject::new("distributed-home-vec-empty-pop-run");
    let source = project.write_source(
        "vec_empty_pop.nct",
        r#"use std/vec.Vec

func main(): i32 {
    var values: Vec<i32> = Vec.empty()
    let unexpected = values.pop() otherwise { return 0 }
    return unexpected
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
fn distributed_std_vec_push_scalar_values_runs() {
    let project = TempProject::new("distributed-home-vec-push-scalar-run");
    let source = project.write_source(
        "vec_push_scalar.nct",
        r#"use std/vec.Vec

func main(): i32! {
    var bytes: Vec<u8> = Vec.empty()
    bytes.push(1)?
    bytes.push(7)?
    if bytes.len() != 2 {
        return 1
    }
    if bytes.capacity() != 2 {
        return 2
    }
    if bytes.view()[0] != 1 {
        return 3
    }
    if bytes.view()[1] != 7 {
        return 4
    }

    var words: Vec<usize> = Vec.empty()
    words.push(11)?
    words.push(31)?
    if words.len() != 2 {
        return 5
    }
    if words.capacity() != 2 {
        return 6
    }
    if words.view()[0] != 11 {
        return 7
    }
    if words.view()[1] != 31 {
        return 8
    }

    var numbers: Vec<i32> = Vec.empty()
    numbers.push(11)?
    numbers.push(42)?
    if numbers.len() != 2 {
        return 9
    }
    if numbers.capacity() != 2 {
        return 10
    }
    if numbers.view()[0] != 11 {
        return 11
    }
    if numbers.view()[1] != 42 {
        return 12
    }

    var flags: Vec<bool> = Vec.empty()
    flags.push(true)?
    flags.push(false)?
    if flags.len() != 2 {
        return 13
    }
    if flags.capacity() != 2 {
        return 14
    }
    if flags.view()[0] != true {
        return 15
    }
    if flags.view()[1] != false {
        return 16
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_push_str_values_runs() {
    let project = TempProject::new("distributed-home-vec-push-str-run");
    let source = project.write_source(
        "vec_push_str.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<&str> = Vec.empty()
    values.push("first")?
    values.push("second")?
    if values.len() != 2 {
        return 1
    }
    if values.capacity() != 2 {
        return 2
    }
    if values.view().len() != 2 {
        return 3
    }
    if values.view()[0] != "first" {
        return 4
    }
    if values.view()[1] != "second" {
        return 5
    }
    let copy: Vec<&str> = Vec.from_slice(&+allocator, values.view())?
    if copy.len() != 2 {
        return 6
    }
    if copy.view()[0] != "first" {
        return 7
    }
    if copy.view()[1] != "second" {
        return 8
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_push_value_control_arguments_runs() {
    let project = TempProject::new("distributed-home-vec-push-value-control-run");
    let source = project.write_source(
        "vec_push_value_control.nct",
        r#"use std/vec.Vec

enum Choice {
    yes
    no
    maybe
}

func main(): i32! {
    let choice = Choice.no

    var bytes: Vec<u8> = Vec.empty()
    bytes.push(if choice is Choice.no { 5 } else { 1 })?
    bytes.push(match choice { Choice.no { 7 } _ { 1 } })?
    if bytes.len() != 2 {
        return 1
    }
    if bytes.view()[0] != 5 {
        return 2
    }
    if bytes.view()[1] != 7 {
        return 3
    }

    var words: Vec<usize> = Vec.empty()
    words.push(match choice { Choice.no { 13 } _ { 1 } })?
    if words.len() != 1 {
        return 4
    }
    if words.view()[0] != 13 {
        return 5
    }

    var flags: Vec<bool> = Vec.empty()
    flags.push(if choice is Choice.no { true } else { false })?
    if flags.len() != 1 {
        return 6
    }
    if flags.view()[0] != true {
        return 7
    }

    var texts: Vec<&str> = Vec.empty()
    texts.push(match choice { Choice.no { "Nocter" } _ { "Other" } })?
    if texts.len() != 1 {
        return 8
    }
    if texts.view()[0] != "Nocter" {
        return 9
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_view_mut_scalar_values_runs() {
    let project = TempProject::new("distributed-home-vec-view-mut-scalar-run");
    let source = project.write_source(
        "vec_view_mut_scalar.nct",
        r#"use std/vec.Vec

func set_first_byte(bytes: &+[u8]): void {
    bytes[0] = 4
    return
}

func main(): i32! {
    var bytes: Vec<u8> = Vec.empty()
    bytes.push(1)?
    bytes.push(2)?
    set_first_byte(bytes.view_mut())
    bytes.view_mut()[1] = 5
    if bytes.view()[0] != 4 {
        return 1
    }
    if bytes.view()[1] != 5 {
        return 2
    }

    var words: Vec<usize> = Vec.empty()
    words.push(11)?
    words.push(12)?
    words.view_mut()[0] = 21
    words.view_mut()[1] = 22
    if words.view()[0] != 21 {
        return 3
    }
    if words.view()[1] != 22 {
        return 4
    }

    var numbers: Vec<i32> = Vec.empty()
    numbers.push(31)?
    numbers.push(32)?
    numbers.view_mut()[0] = 41
    numbers.view_mut()[1] = 42
    if numbers.view()[0] != 41 {
        return 5
    }
    if numbers.view()[1] != 42 {
        return 6
    }

    var flags: Vec<bool> = Vec.empty()
    flags.push(true)?
    flags.push(false)?
    flags.view_mut()[0] = false
    flags.view_mut()[1] = true
    if flags.view()[0] != false {
        return 7
    }
    if flags.view()[1] != true {
        return 8
    }

    var texts: Vec<&str> = Vec.empty()
    texts.push("before")?
    texts.push("old")?
    texts.view_mut()[0] = "after"
    texts.view_mut()[1] = "new"
    if texts.view()[0] != "after" {
        return 9
    }
    if texts.view()[1] != "new" {
        return 10
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_view_mut_integer_compound_assignments_run() {
    let project = TempProject::new("distributed-home-vec-view-mut-compound-run");
    let source = project.write_source(
        "vec_view_mut_compound.nct",
        r#"use std/vec.Vec

func one(): i32 {
    return 1
}

func main(): i32! {
    var words: Vec<usize> = Vec.empty()
    words.push(40)?
    words.push(47)?
    words.view_mut()[0] += 2
    words.view_mut()[1] %= 5
    if words.view()[0] != 42 {
        return 1
    }
    if words.view()[1] != 2 {
        return 2
    }

    var numbers: Vec<i32> = Vec.empty()
    numbers.push(40)?
    numbers.push(8)?
    numbers.view_mut()[0] += one()
    numbers.view_mut()[1] *= 5
    numbers.view_mut()[1] -= 10
    if numbers.view()[0] != 41 {
        return 3
    }
    if numbers.view()[1] != 30 {
        return 4
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[test]
fn distributed_std_vec_fields_are_private() {
    let project = TempProject::new("distributed-home-vec-fields-private");
    let source = project.write_source(
        "vec_fields_private.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec<usize> {
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

#[test]
fn distributed_std_vec_builds_copy_aggregate_push() {
    let project = TempProject::new("distributed-home-vec-aggregate-push-boundary");
    let source = project.write_source(
        "vec_aggregate_push_boundary.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { value: 1 })?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_aggregate_push_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec push"
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_free_push() {
    let project = TempProject::new("distributed-home-vec-aggregate-free-push-boundary");
    let source = project.write_source(
        "vec_aggregate_free_push_boundary.nct",
        r#"use std/vec.{Vec, push}

copy struct Pair {
    pub value: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    push(&+values, Pair { value: 1 })?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_aggregate_free_push_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for free copy aggregate Vec push"
    );
}

#[test]
fn distributed_std_vec_builds_view_mut_index_method_borrow_argument() {
    let project = TempProject::new("distributed-home-vec-view-mut-index-method-borrow");
    let source = project.write_source(
        "vec_view_mut_index_method_borrow.nct",
        r#"use std/vec.Vec

copy struct Checker {
    seed: i32
}

impl Checker {
    method &self.touch(value: &+i32): void {
        return
    }
}

func main(): void! {
    var values: Vec<i32> = Vec.empty()
    values.push(1)?
    let checker = Checker { seed: 0 }
    checker.touch(&+values.view_mut()[0])
    return
}
"#,
    );
    let executable = project.root().join("vec_view_mut_index_method_borrow");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for slice index read-write borrow arguments"
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_with_capacity() {
    let project = TempProject::new("distributed-home-vec-aggregate-with-capacity-boundary");
    let source = project.write_source(
        "vec_aggregate_with_capacity_boundary.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32! {
    var allocator = page_allocator()
    let values: Vec<Pair> = Vec.with_capacity(&+allocator, 1)?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_aggregate_with_capacity_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec.with_capacity"
    );
}

#[test]
fn distributed_std_vec_builds_cross_source_generic_copy_aggregate_with_capacity() {
    let project =
        TempProject::new("distributed-home-vec-cross-source-generic-copy-aggregate-capacity");
    project.write_source(
        "types.nct",
        r#"pub copy struct Pair {
    pub value: i32
}
"#,
    );
    project.write_source(
        "factory.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

pub func make<T>(seed: T): Vec<T>! {
    var allocator = page_allocator()
    return Vec.with_capacity(&+allocator, 1)?
}
"#,
    );
    let source = project.write_source(
        "app.nct",
        r#"use std/vec.Vec
use ./factory.make
use ./types.Pair

func main(): i32! {
    let values: Vec<Pair> = make(Pair { value: 1 })?
    return 0
}
"#,
    );
    let executable = project.root().join("app");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for cross-source generic copy aggregate Vec.with_capacity"
    );
}

#[test]
fn distributed_std_vec_builds_cross_source_generic_non_copy_aggregate_with_capacity() {
    let project =
        TempProject::new("distributed-home-vec-cross-source-generic-non-copy-aggregate-capacity");
    project.write_source(
        "types.nct",
        r#"pub struct Text {
    pub value: &str
}
"#,
    );
    project.write_source(
        "factory.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

pub func make<T>(seed: T): Vec<T>! {
    var allocator = page_allocator()
    return Vec.with_capacity(&+allocator, 1)?
}
"#,
    );
    let source = project.write_source(
        "app.nct",
        r#"use std/vec.Vec
use ./factory.make
use ./types.Text

func main(): i32! {
    let values: Vec<Text> = make(Text { value: "x" })?
    return 0
}
"#,
    );
    let executable = project.root().join("app");

    let output = nocter_build(&project, &source, &executable);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(
        executable.exists(),
        "build should produce an executable for non-copy aggregate Vec.with_capacity"
    );
}

#[test]
fn distributed_std_vec_builds_cross_source_non_copy_generic_copy_struct_with_capacity() {
    let project =
        TempProject::new("distributed-home-vec-cross-source-non-copy-generic-copy-struct-capacity");
    project.write_source(
        "types.nct",
        r#"pub struct Text {
    pub value: &str
}

pub copy struct Box<T> {
    pub value: T
}
"#,
    );
    project.write_source(
        "factory.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

pub func make<T>(seed: T): Vec<T>! {
    var allocator = page_allocator()
    return Vec.with_capacity(&+allocator, 1)?
}
"#,
    );
    let source = project.write_source(
        "app.nct",
        r#"use std/vec.Vec
use ./factory.make
use ./types.{Box, Text}

func main(): i32! {
    let values: Vec<Box<Text>> = make(Box<Text> { value: Text { value: "x" } })?
    return 0
}
"#,
    );
    let executable = project.root().join("app");

    let output = nocter_build(&project, &source, &executable);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(
        executable.exists(),
        "build should produce an executable for non-copy generic aggregate Vec.with_capacity"
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_reserve() {
    let project = TempProject::new("distributed-home-vec-aggregate-reserve-boundary");
    let source = project.write_source(
        "vec_aggregate_reserve_boundary.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.reserve(1)?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_aggregate_reserve_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec.reserve"
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_from_slice() {
    let project = TempProject::new("distributed-home-vec-aggregate-from-slice-boundary");
    let source = project.write_source(
        "vec_aggregate_from_slice_boundary.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32! {
    var allocator = page_allocator()
    let values: Vec<Pair> = Vec.empty()
    let copy = Vec.from_slice(&+allocator, values.view())?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_aggregate_from_slice_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec.from_slice"
    );
}

#[test]
fn distributed_std_vec_rejects_non_copy_aggregate_from_slice() {
    let project = TempProject::new("distributed-home-vec-non-copy-from-slice-boundary");
    let source = project.write_source(
        "vec_non_copy_from_slice_boundary.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

struct Text {
    value: &str
}

func main(): i32! {
    var allocator = page_allocator()
    var values: Vec<Text> = Vec.empty()
    let value = Text { value: "owned" }
    values.push(move value)?
    let copy = Vec.from_slice(&+allocator, values.view())?
    return 0
}
"#,
    );
    let executable = project.root().join("vec_non_copy_from_slice_boundary");

    let output = nocter_build(&project, &source, &executable);

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0435]")
            && stderr.contains("`Vec.from_slice` with a non-copy element type"),
        "expected Vec.from_slice copyability diagnostic, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not produce an executable for non-copy Vec.from_slice"
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_view_index() {
    let project = TempProject::new("distributed-home-vec-aggregate-view-index-boundary");
    let source = project.write_source(
        "vec_aggregate_view_index_boundary.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32 {
    let values: Vec<Pair> = Vec.empty()
    let first = values.view()[0]
    return first.value
}
"#,
    );
    let executable = project.root().join("vec_aggregate_view_index_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec.view index"
    );
}

#[test]
fn distributed_std_vec_runs_copy_aggregate_view_index() {
    let project = TempProject::new("distributed-home-vec-aggregate-view-index-run");
    let source = project.write_source(
        "vec_aggregate_view_index_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 7, right: 11 })?
    let first = values.view()[0]
    return first.left + first.right
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(18),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_vec_runs_bound_copy_aggregate_view_index() {
    let project = TempProject::new("distributed-home-vec-bound-aggregate-view-index-run");
    let source = project.write_source(
        "vec_bound_aggregate_view_index_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 9, right: 14 })?
    let view = values.view()
    let first = view[0]
    return first.left + first.right
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_vec_builds_copy_aggregate_view_mut_index_assignment() {
    let project = TempProject::new("distributed-home-vec-aggregate-view-mut-index-assign-boundary");
    let source = project.write_source(
        "vec_aggregate_view_mut_index_assign_boundary.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub value: i32
}

func main(): i32 {
    var values: Vec<Pair> = Vec.empty()
    values.view_mut()[0] = Pair { value: 1 }
    return 0
}
"#,
    );
    let executable = project
        .root()
        .join("vec_aggregate_view_mut_index_assign_boundary");

    let output = nocter_build(&project, &source, &executable);

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
    assert!(
        executable.exists(),
        "build should produce an executable for copy aggregate Vec.view_mut index assignment"
    );
}

#[test]
fn distributed_std_vec_runs_copy_aggregate_view_mut_index_assignment() {
    let project = TempProject::new("distributed-home-vec-aggregate-view-mut-index-assign-run");
    let source = project.write_source(
        "vec_aggregate_view_mut_index_assign_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 1, right: 2 })?
    values.view_mut()[0] = Pair { left: 13, right: 5 }
    let first = values.view()[0]
    return first.left - first.right
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(8),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_vec_runs_copy_aggregate_slice_parameter_indexing() {
    let project = TempProject::new("distributed-home-vec-aggregate-slice-parameter-index-run");
    let source = project.write_source(
        "vec_aggregate_slice_parameter_index_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

func replace(view: &+[Pair]): void {
    view[0] = Pair { left: 17, right: 6 }
    return
}

func sum(view: &[Pair]): i32 {
    let first = view[0]
    return first.left + first.right
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 1, right: 2 })?
    let mutable_view = values.view_mut()
    replace(mutable_view)
    let view = values.view()
    return sum(view)
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_vec_runs_copy_aggregate_slice_field_indexing() {
    let project = TempProject::new("distributed-home-vec-aggregate-slice-field-index-run");
    let source = project.write_source(
        "vec_aggregate_slice_field_index_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

copy struct Holder {
    pub view: &[Pair]
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 5, right: 18 })?
    let holder = Holder { view: values.view() }
    let first = holder.view[0]
    return first.left + first.right
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_vec_runs_copy_aggregate_slice_field_index_assignment() {
    let project = TempProject::new("distributed-home-vec-aggregate-slice-field-index-assign-run");
    let source = project.write_source(
        "vec_aggregate_slice_field_index_assign_run.nct",
        r#"use std/vec.Vec

copy struct Pair {
    pub left: i32
    pub right: i32
}

struct Holder {
    pub view: &+[Pair]
}

func main(): i32! {
    var values: Vec<Pair> = Vec.empty()
    values.push(Pair { left: 1, right: 2 })?
    var holder = Holder { view: values.view_mut() }
    holder.view[0] = Pair { left: 19, right: 4 }
    let first = values.view()[0]
    return first.left + first.right
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
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
    let text = String { len: 0 }
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
fn distributed_std_mem_raw_buffer_literal_is_not_public_api() {
    let project = TempProject::new("distributed-home-raw-buffer-literal-private");
    let source = project.write_source(
        "raw_buffer_literal_private.nct",
        r#"use std/mem.RawBuffer
use std/ptr.from_ref

func main(): i32 {
    let byte: u8 = 0
    let buffer = RawBuffer {
        ptr: from_ref(&byte),
        len: 1,
        align: 1,
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
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("E0377") && stderr.contains("not visible here"),
        "expected private RawBuffer field diagnostic, got:\n{stderr}"
    );
}

#[test]
fn distributed_std_mem_raw_buffer_fields_are_not_public_api() {
    let project = TempProject::new("distributed-home-raw-buffer-fields-private");
    let source = project.write_source(
        "raw_buffer_fields_private.nct",
        r#"use std/mem.{alloc, page_allocator}

func main(): i32! {
    var allocator = page_allocator()
    let buffer = alloc(&+allocator, 1, 1)?
    let length = buffer.len
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
        "expected private RawBuffer field diagnostic, got:\n{stderr}"
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
fn distributed_std_allocator_methods_run() {
    let project = TempProject::new("distributed-home-allocator-methods-run");
    fs::write(project.root().join("input.txt"), b"OK").unwrap();
    let source = project.write_source(
        "allocator_methods.nct",
        r#"use std/io.File
use std/mem.{Allocator, RawBuffer, page_allocator}

func allocate(allocator: &+Allocator): RawBuffer! {
    return allocator.alloc(2, 1)?
}

func release(allocator: &+Allocator, buffer: RawBuffer): void {
    allocator.free(move buffer)
    return
}

func main(): i32! {
    var allocator = page_allocator()
    var buffer = allocate(&+allocator)?
    var input = File.open("input.txt")?
    let count = input.read(buffer.bytes_mut())?
    if count != 2 {
        return 1
    }
    if buffer.bytes()[0] != 79 {
        return 2
    }
    let bytes = buffer.prefix(2)?
    if bytes[1] != 75 {
        return 3
    }
    release(&+allocator, move buffer)
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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_allocator_layout_zero_and_grow_run() {
    let project = TempProject::new("distributed-home-allocator-layout-grow-run");
    let source = project.write_source(
        "allocator_layout_grow.nct",
        r#"use std/mem.{Layout, alloc_layout, free, page_allocator}

func main(): i32! {
    var allocator = page_allocator()
    let empty_layout = Layout.new(0, 16)?
    if empty_layout.size() != 0 {
        return 1
    }
    if empty_layout.align() != 16 {
        return 2
    }
    var empty = alloc_layout(&+allocator, empty_layout)?
    if empty.bytes().len() != 0 {
        return 3
    }
    allocator.grow(&+empty, 2)?
    empty.bytes_mut()[0] = 20
    empty.bytes_mut()[1] = 22
    allocator.grow(&+empty, 8)?
    if empty.bytes().len() != 8 {
        return 4
    }
    if empty.bytes()[0] != 20 {
        return 5
    }
    if empty.bytes()[1] != 22 {
        return 6
    }
    free(&+allocator, move empty)
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_allocator_rejects_non_power_of_two_alignment() {
    let project = TempProject::new("distributed-home-allocator-invalid-alignment-run");
    let source = project.write_source(
        "allocator_invalid_alignment.nct",
        r#"use std/mem.{alloc, page_allocator}

func main(): void! {
    var allocator = page_allocator()
    let buffer = alloc(&+allocator, 1, 3)?
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"std.mem.invalid_argument: invalid allocation request\n"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_allocator_failed_grow_preserves_the_old_buffer() {
    let project = TempProject::new("distributed-home-allocator-failed-grow-run");
    let source = project.write_source(
        "allocator_failed_grow.nct",
        r#"use std/mem.{Allocator, RawBuffer, alloc, page_allocator}

func grow_huge(allocator: &+Allocator, buffer: &+RawBuffer): usize! {
    allocator.grow(buffer, 18446744073709551615)?
    return buffer.bytes().len()
}

func preserved(buffer: &RawBuffer): i32 {
    if buffer.bytes().len() != 1 {
        return 1
    }
    if buffer.bytes()[0] != 42 {
        return 2
    }
    return 42
}

func main(): i32! {
    var allocator = page_allocator()
    var buffer = alloc(&+allocator, 1, 1)?
    buffer.bytes_mut()[0] = 42
    let size = grow_huge(&+allocator, &+buffer) catch error {
        return preserved(&buffer)
    }
    return 3
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
fn distributed_std_allocator_reports_out_of_memory() {
    let project = TempProject::new("distributed-home-allocator-out-of-memory-run");
    let source = project.write_source(
        "allocator_out_of_memory.nct",
        r#"use std/mem.{alloc, page_allocator}

func main(): void! {
    var allocator = page_allocator()
    let buffer = alloc(&+allocator, 18446744073709551615, 1)?
}
"#,
    );

    let output = nocter_run(&project, &source);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"std.mem.out_of_memory: allocation failed\n");
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
    let file = File { close_on_drop: false }
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
fn distributed_ptr_raw_helpers_are_not_public_api() {
    for helper in [
        "from_addr",
        "pointee_size",
        "pointee_align",
        "copy_str_to_ptr",
        "copy_ptr_to_ptr",
        "store_u8_to_ptr",
        "store_value_to_ptr",
        "drop_value_at_ptr",
        "take_value_at_ptr",
        "str_from_raw_parts",
        "slice_from_raw_parts",
        "slice_from_raw_parts_mut",
        "slice_from_raw_parts_value",
        "slice_from_raw_parts_value_mut",
    ] {
        let project = TempProject::new(&format!("distributed-home-ptr-{helper}-private"));
        let source = project.write_source(
            &format!("ptr_{helper}_private.nct"),
            &format!(
                r#"use std/ptr.{helper}

func main(): i32 {{
    return 0
}}
"#
            ),
        );

        let output = nocter_check(&project, &source);
        assert_pub_nocter_visibility_rejected(&output, helper);
    }
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
    let values_len: usize = values.len()
    if values_len == 0 {
        return 1
    }
    let view = values.view()
    let view_len: usize = view.len()
    if view_len != values_len {
        return 2
    }
    let executable = view[0]
    let executable_len: usize = executable.len()
    if executable_len == 0 {
        return 3
    }
    let first_byte: u8 = executable[0]
    if first_byte == 0 {
        return 4
    }
    return 0
}
"#,
    );
    let executable = project.root().join("process_args_build");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_process_args_returns_argv_vector() {
    let project = TempProject::new("distributed-home-process-args-runtime");
    let source = project.write_source(
        "process_args_runtime.nct",
        r#"use std/process.args

func main(): i32! {
    let values = args()?
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
fn distributed_std_string_failed_growth_preserves_contents() {
    let project = TempProject::new("distributed-home-string-failed-growth-run");
    let source = project.write_source(
        "string_failed_growth.nct",
        r#"use std/mem.page_allocator

func grow_huge(text: &+String): void! {
    text.reserve(18446744073709551611)?
    return
}

func preserved(text: &String): i32 {
    if text.view() != "keep" {
        return 1
    }
    if text.len() != 4 {
        return 2
    }
    if text.capacity() != 4 {
        return 3
    }
    return 42
}

func main(): i32! {
    var allocator = page_allocator()
    var text = String.from_str(&+allocator, "keep")?
    grow_huge(&+text) catch error {
        return preserved(&text)
    }
    return 4
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
    fs::write(
        home.join("LICENSE"),
        fs::read_to_string(distributed_home().join("LICENSE")).unwrap(),
    )
    .unwrap();
    fs::write(
        home.join("NOTICE"),
        fs::read_to_string(distributed_home().join("NOTICE")).unwrap(),
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

fn assert_pub_nocter_visibility_rejected(output: &Output, imported_name: &str) {
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
        stderr.contains("E0412")
            && stderr.contains("pub(nocter)")
            && stderr.contains(imported_name),
        "expected pub(nocter) visibility diagnostic for `{imported_name}`, got:\n{stderr}"
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
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(write_test_distributed_home).clone()
}

fn write_test_distributed_home() -> PathBuf {
    let root = std::env::temp_dir().join(unique_name("distributed-home-image"));
    let home = root.join(".nocter");

    fs::create_dir_all(&home).unwrap();
    fs::copy(
        development_root().join("packaging/VERSION"),
        home.join("VERSION"),
    )
    .unwrap();
    fs::copy(
        development_root().join("packaging/MANIFEST.json"),
        home.join("MANIFEST.json"),
    )
    .unwrap();
    fs::copy(
        development_root().parent().unwrap().join("LICENSE"),
        home.join("LICENSE"),
    )
    .unwrap();
    fs::copy(
        development_root().parent().unwrap().join("NOTICE"),
        home.join("NOTICE"),
    )
    .unwrap();
    let compiler = home.join("nocter");
    fs::copy(NOCTER, &compiler).unwrap();
    fs::set_permissions(&compiler, fs::metadata(NOCTER).unwrap().permissions()).unwrap();
    copy_tree(&development_root().join("std"), &home.join("std"));

    home
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let target = destination.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn development_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live below development root")
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
