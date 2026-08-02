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
