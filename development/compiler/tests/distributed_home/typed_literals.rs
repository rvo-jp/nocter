use super::*;

#[test]
fn distributed_std_typed_literals_check_from_packaged_home() {
    let project = TempProject::new("distributed-home-typed-literal-check");
    let source = project.write_source(
        "typed_literals.nct",
        r#"use std/vec.Vec

func values(): Vec<i32> {
    return Vec [1, 2, 3]
}

func text(): String {
    return String "hello"
}

func main(): i32 {
    let empty: Vec<i32> = Vec []
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_and_string_typed_literals_run() {
    let project = TempProject::new("distributed-home-typed-literal-run");
    let source = project.write_source(
        "typed_literals_run.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec [10, 20, 12]
    if values.len() != 3 {
        return 1
    }
    if values.view()[0] != 10 || values.view()[1] != 20 || values.view()[2] != 12 {
        return 2
    }

    let empty: Vec<i32> = Vec []
    if !empty.is_empty() || empty.capacity() != 0 {
        return 3
    }

    let text = String "hello"
    if text.view() != "hello" || text.len() != 5 {
        return 4
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_literal_moves_owned_strings_once() {
    let project = TempProject::new("distributed-home-move-only-typed-literal-run");
    let source = project.write_source(
        "move_only_typed_literals.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let first = String "first"
    let second = String "second"
    var values = Vec [move first, move second]
    if values.len() != 2 {
        return 1
    }
    values.clear()
    if !values.is_empty() {
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literals_use_lexical_region_context_and_release_normally() {
    let project = TempProject::new("distributed-home-region-typed-literal-run");
    let source = project.write_source(
        "region_typed_literals.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func main(): i32 {
    let page = page_allocator()
    region temp using page {
        let values = Vec [1, 2, 3]
        let text = String "region"
        if values.len() != 3 || text.view() != "region" {
            return 1
        }
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn explicit_literal_context_overrides_a_lexical_region() {
    let project = TempProject::new("distributed-home-explicit-typed-literal-context-run");
    let source = project.write_source(
        "explicit_typed_literal_context.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func make_values(): Vec<i32> {
    let root = page_allocator()
    let arena = page_allocator()
    region temp using arena {
        return Vec [20, 22] using root
    }
}

func make_text(): String {
    let root = page_allocator()
    let arena = page_allocator()
    region temp using arena {
        return String "explicit" using root
    }
}

func main(): i32 {
    let values = make_values()
    let text = make_text()
    if values.len() != 2 || values.view()[0] != 20 || values.view()[1] != 22 {
        return 1
    }
    if text.view() != "explicit" {
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literal_body_exit_drops_unconsumed_elements_in_reverse_order() {
    let project = TempProject::new("distributed-home-typed-literal-pack-drop-run");
    let source = project.write_source(
        "typed_literal_pack_drop.nct",
        r#"use std/io.print

struct Token {
    label: &str
}

impl Token {
    drop &+self {
        print(self.label)!
        return
    }
}

struct Sink {
    code: i32
}

literal Sink [](...items: Token): Self {
    return Sink { code: 0 }
}

func main(): i32 {
    let sink = Sink [Token { label: "A" }, Token { label: "B" }, Token { label: "C" }]
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
    assert_eq!(output.stdout, b"CBA");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literal_allocation_failure_uses_stable_aborting_status() {
    let project = TempProject::new("distributed-home-typed-literal-allocation-abort-run");
    let source = project.write_source(
        "typed_literal_allocation_abort.nct",
        r#"use std/vec.Vec

struct Exhausted {
    code: i32
}

literal Exhausted [](...items: i32): Self {
    let impossible: Vec<u8> = Vec.with_capacity(18446744073709551615)
    return Exhausted { code: 0 }
}

func main(): i32 {
    let exhausted = Exhausted [1]
    return 1
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(output.status.code(), Some(70));
    assert!(output.stdout.is_empty(), "expected empty stdout");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}

#[test]
fn typed_literal_region_origin_cannot_escape_through_an_aggregate() {
    let project = TempProject::new("distributed-home-typed-literal-region-escape-check");
    let source = project.write_source(
        "typed_literal_region_escape.nct",
        r#"use std/mem.page_allocator

struct Holder {
    text: String
}

func leak(): Holder {
    let arena = page_allocator()
    region temporary using arena {
        return Holder { text: String "escape" }
    }
}

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
    assert!(output.stdout.is_empty(), "expected empty stdout");
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0436]").count(), 1, "{stderr}");
    assert!(
        stderr.contains("region `temporary`") && stderr.contains("region ends before"),
        "expected source-backed literal origin details:\n{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literal_region_release_unmaps_literal_owned_storage() {
    let project = TempProject::new("distributed-home-typed-literal-region-unmap-run");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);
    let vec_module = home.join("std/vec.nct");
    let vec_source = fs::read_to_string(&vec_module).unwrap();
    fs::write(
        &vec_module,
        format!(
            "{vec_source}\n\npub func storage_address<T>(values: &Vec<T>): usize {{\n    return addr(values.storage.ptr)\n}}\n"
        ),
    )
    .unwrap();
    fs::write(
        home.join("std/literal_region_probe.nct"),
        r#"use std/os.syscall3

pub func is_mapped(address: usize): bool {
    let page_start = address / 16384 * 16384
    let result = syscall3(0x0200004a, page_start, 16384, 3)
    return result.errno == 0
}
"#,
    )
    .unwrap();
    let source = project.write_source(
        "typed_literal_region_unmap.nct",
        r#"use std/literal_region_probe.is_mapped
use std/mem.page_allocator
use std/vec.{Vec, storage_address}

func main(): i32 {
    var arena = page_allocator()
    var address: usize = 0
    region temporary using arena {
        let values = Vec [10, 20, 12]
        address = storage_address(&values)
        if !is_mapped(address) {
            return 1
        }
    }
    if is_mapped(address) {
        return 2
    }
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
