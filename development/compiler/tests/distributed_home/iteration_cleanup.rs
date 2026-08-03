use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_collection_for_exhaustion_drops_each_owned_item_once() {
    let project = TempProject::new("distributed-home-collection-for-exhaustion-cleanup-run");
    let source = project.write_source(
        "collection_for_exhaustion_cleanup_run.nct",
        &token_program(
            r#"    let values = tokens()
    for token in move values {
        drop token
    }
    return 42"#,
        ),
    );

    assert_token_run(&project, &source, b"ABCD");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_collection_for_break_drops_item_then_remaining_suffix() {
    let project = TempProject::new("distributed-home-collection-for-break-cleanup-run");
    let source = project.write_source(
        "collection_for_break_cleanup_run.nct",
        &token_program(
            r#"    let values = tokens()
    for token in move values {
        drop token
        break
    }
    return 42"#,
        ),
    );

    assert_token_run(&project, &source, b"ADCB");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_collection_for_return_cleans_item_before_iterator() {
    let project = TempProject::new("distributed-home-collection-for-return-cleanup-run");
    let source = project.write_source(
        "collection_for_return_cleanup_run.nct",
        &token_program(
            r#"    let values = tokens()
    for token in move values {
        drop token
        return 42
    }
    return 1"#,
        ),
    );

    assert_token_run(&project, &source, b"ADCB");
}

#[test]
fn distributed_std_region_iterators_cannot_escape_their_storage_region() {
    let project = TempProject::new("distributed-home-iterator-region-escape-check");
    let source = project.write_source(
        "iterator_region_escape.nct",
        r#"use std/iter.ViewIter
use std/mem.page_allocator
use std/vec.Vec
use std/vec_into_iter.VecIntoIter

func leak_readonly(): ViewIter<i32> {
    let arena = page_allocator()
    region temporary using arena {
        let values = Vec [1, 2, 3]
        return values.iter()
    }
}

func leak_owned(): VecIntoIter<i32> {
    let arena = page_allocator()
    region temporary using arena {
        let values = Vec [1, 2, 3]
        return (move values).into_iter()
    }
}

func leak_element(): &i32 {
    let arena = page_allocator()
    region temporary using arena {
        let values = Vec [1, 2, 3]
        var iterator = values.iter()
        return iterator.next() otherwise { return &values.view()[0] }
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
    let stderr = text(&output.stderr);
    // `leak_element` has two independently escaping paths: the iterator
    // success value and the fallback borrow. Precise literal allocation
    // provenance must diagnose both rather than hiding one behind a merged
    // unknown summary.
    assert_eq!(stderr.matches("error[E0436]").count(), 4, "{stderr}");
    assert!(stderr.contains("region `temporary`"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_owned_iterator_exhaustion_moves_each_element_once() {
    let project = TempProject::new("distributed-home-owned-iterator-exhaustion-cleanup-run");
    let source = project.write_source(
        "owned_iterator_exhaustion_cleanup_run.nct",
        &token_program(
            r#"    let values = tokens()
    var iterator = (move values).into_iter()
    loop {
        let token = iterator.next() otherwise { break }
        drop token
    }
    return 42"#,
        ),
    );

    assert_token_run(&project, &source, b"ABCD");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_owned_iterator_break_drops_the_full_remaining_range() {
    let project = TempProject::new("distributed-home-owned-iterator-break-cleanup-run");
    let source = project.write_source(
        "owned_iterator_break_cleanup_run.nct",
        &token_program(
            r#"    loop {
        let values = tokens()
        let iterator = (move values).into_iter()
        break
    }
    return 42"#,
        ),
    );

    assert_token_run(&project, &source, b"DCBA");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_owned_iterator_propagation_drops_the_remaining_range() {
    let project = TempProject::new("distributed-home-owned-iterator-propagation-cleanup-run");
    let source = project.write_source(
        "owned_iterator_propagation_cleanup_run.nct",
        r#"use std/io.print
use std/vec.Vec

struct Token {
    label: &str
}

impl Token {
    drop &+self {
        print(self.label)!
        return
    }
}

func fail(): void! {
    return Error.new("test.failure", "expected")
}

func propagate(): i32! {
    let values = Vec [
        Token { label: "A" },
        Token { label: "B" },
        Token { label: "C" },
        Token { label: "D" },
    ]
    var iterator = (move values).into_iter()
    let first = iterator.next() otherwise { return 1 }
    drop first
    fail()?
    return 1
}

func main(): i32 {
    return propagate() catch expected_failure {
        return 42
    }
}
"#,
    );

    assert_token_run(&project, &source, b"ADCB");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn token_program(main_body: &str) -> String {
    format!(
        r#"use std/io.print
use std/vec.Vec

struct Token {{
    label: &str
}}

impl Token {{
    drop &+self {{
        print(self.label)!
        return
    }}
}}

func tokens(): Vec<Token> {{
    return Vec [
        Token {{ label: "A" }},
        Token {{ label: "B" }},
        Token {{ label: "C" }},
        Token {{ label: "D" }},
    ]
}}

func main(): i32 {{
{main_body}
}}
"#
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_token_run(project: &TempProject, source: &Path, expected_stdout: &[u8]) {
    let output = nocter_run(project, source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.is_empty());
}
