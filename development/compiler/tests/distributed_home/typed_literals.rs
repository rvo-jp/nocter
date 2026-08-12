use super::*;

fn typed_literal_lsp_position(text: &str, offset: usize) -> Value {
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    json!({"line":line,"character":text[line_start..offset].chars().count()})
}

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

#[test]
fn distributed_std_sequence_spread_modes_check_from_packaged_home() {
    let project = TempProject::new("distributed-home-sequence-spread-check");
    let source = project.write_source(
        "sequence_spread.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let copied = Vec [1, 2, 3]
    let with_copies = Vec [0, ...copied, 4]

    let borrowed = Vec [String "a", String "b"]
    let with_borrows: Vec<&String> = Vec [...&borrowed]

    let owned = Vec [String "c", String "d"]
    let with_owned = Vec [String "b", ...move owned, String "e"]
    if with_copies.len() != 5 || with_borrows.len() != 2 || with_owned.len() != 5 {
        return 1
    }
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_sequence_spread_rejects_mutable_expansion() {
    let project = TempProject::new("distributed-home-sequence-spread-mutable-rejection");
    let source = project.write_source(
        "sequence_spread_mutable_rejection.nct",
        r#"use std/vec.Vec

func main(): i32 {
    var source = Vec [1, 2, 3]
    let invalid = Vec [...&+source]
    return invalid.len()
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0524]"), "{stderr}");
    assert!(stderr.contains("mutable sequence spread"), "{stderr}");
    assert!(stderr.contains("for item in &+collection"), "{stderr}");
}

#[test]
fn distributed_std_copy_spread_rejects_move_only_elements() {
    let project = TempProject::new("distributed-home-sequence-spread-copy-rejection");
    let source = project.write_source(
        "sequence_spread_copy_rejection.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let owned = Vec [String "a"]
    let invalid = Vec [...owned]
    return invalid.len()
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0524]"), "{stderr}");
    assert!(stderr.contains("...move source"), "{stderr}");
}

#[test]
fn distributed_std_sequence_spread_rejects_unknown_size_iterators() {
    let project = TempProject::new("distributed-home-sequence-spread-exact-size-rejection");
    let source = project.write_source(
        "sequence_spread_exact_size_rejection.nct",
        r#"use std/iter.Iterator
use std/vec.Vec

struct Stream {
    done: bool
}

conform Iterator for Stream {
    type Item = i32

    method &+self.next(): i32? {
        if self.done {
            return none
        }
        self.done = true
        return 1
    }
}

func main(): i32 {
    let stream = Stream { done: false }
    let invalid = Vec [...move stream]
    return invalid.len()
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0524]"), "{stderr}");
    assert!(stderr.contains("exact remaining element count"), "{stderr}");
}

#[test]
fn distributed_std_sequence_spread_transfers_owned_source_once() {
    let project = TempProject::new("distributed-home-sequence-spread-move-check");
    let source = project.write_source(
        "sequence_spread_move.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let source = Vec [String "owned"]
    let moved = Vec [...move source]
    return source.len()
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0385]"), "{stderr}");
    assert!(stderr.contains("source"), "{stderr}");
}

#[test]
fn distributed_lsp_exposes_sequence_spread_plans_and_recovers_missing_sources() {
    let project = TempProject::new("distributed-home-sequence-spread-lsp");
    let valid_text = r#"use std/vec.Vec

func run(copy_source: Vec<i32>, borrow_source: Vec<String>, move_source: Vec<String>): void {
    let copied = Vec [0, ...copy_source]
    let borrowed: Vec<&String> = Vec [...&borrow_source]
    let moved = Vec [...move move_source]
    return
}
"#;
    let incomplete_text = r#"use std/vec.Vec

func run(values: Vec<i32>): void {
    let copied = Vec [...
    return
}
"#;
    let source = project.write_source("sequence_spread_lsp.nct", valid_text);
    let uri = file_uri(&source);
    let copy_offset = valid_text.find("copy_source]").unwrap();
    let operator_offset = valid_text.find("...copy_source").unwrap();
    let borrow_offset = valid_text.find("borrow_source]").unwrap();
    let move_offset = valid_text.find("move_source]").unwrap();
    let incomplete_offset = incomplete_text.find("...\n").unwrap() + 3;
    let output = nocter_lsp(
        &distributed_home().join("nocter"),
        project.root(),
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":uri,"languageId":"nocter","version":1,"text":valid_text}}
            }),
            json!({
                "jsonrpc":"2.0","id":2,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,copy_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":3,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,borrow_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":4,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,move_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":5,"method":"textDocument/semanticTokens/full",
                "params":{"textDocument":{"uri":uri}}
            }),
            json!({
                "jsonrpc":"2.0","id":8,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,operator_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":9,"method":"textDocument/completion",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,copy_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":10,"method":"textDocument/signatureHelp",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(valid_text,copy_offset)}
            }),
            json!({
                "jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":incomplete_text}]}
            }),
            json!({
                "jsonrpc":"2.0","id":6,"method":"textDocument/completion",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(incomplete_text,incomplete_offset)}
            }),
            json!({"jsonrpc":"2.0","id":7,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let frames = read_frames(&output.stdout);
    let response = |id| {
        frames
            .iter()
            .find(|message| message["id"] == id)
            .unwrap_or_else(|| panic!("missing LSP response {id}: {frames:#?}"))
    };
    for (id, mode) in [
        (2, "copy from readonly iteration"),
        (8, "copy from readonly iteration"),
        (3, "readonly reference spread"),
        (4, "owned element transfer"),
    ] {
        let markdown = response(id)["result"]["contents"]["value"]
            .as_str()
            .expect("expected spread hover");
        assert!(markdown.contains(mode), "{markdown}");
        assert!(markdown.contains("Exact-count target"), "{markdown}");
        assert!(markdown.contains("step target"), "{markdown}");
    }
    assert!(
        !response(5)["result"]["data"]
            .as_array()
            .expect("expected semantic tokens")
            .is_empty()
    );
    let source_completion = response(9)["result"]["items"]
        .as_array()
        .expect("expected spread source completion items");
    let copy_source = source_completion
        .iter()
        .find(|item| item["label"].as_str() == Some("copy_source"))
        .expect("expected copy source completion");
    assert!(
        copy_source["sortText"]
            .as_str()
            .is_some_and(|sort| sort.starts_with("000-")),
        "copy source completion: {copy_source:#?}"
    );
    assert_eq!(
        response(10)["result"]["signatures"][0]["label"],
        "literal Vec<i32> [](...items: i32): Vec<i32>"
    );
    let completion = response(6)["result"]["items"]
        .as_array()
        .expect("expected recovered completion items");
    assert!(
        completion
            .iter()
            .any(|item| item["label"].as_str() == Some("values")),
        "completion: {completion:#?}"
    );
}

#[test]
fn distributed_lsp_keeps_implicit_sequence_spread_effects_out_of_source_contracts() {
    let project = TempProject::new("distributed-home-sequence-spread-allocation-lsp");
    let source_text = r#"use std/iter.{ExactSizeIterator, Iterator}
use std/vec.Vec

struct AllocatingCollection { end: usize }

struct AllocatingIter {
    next_value: usize
    end: usize
}

instance AllocatingCollection {
    operator (...self): AllocatingIter {
        let scratch = Vec [0]
        drop scratch
        return AllocatingIter { next_value: 0, end: self.end }
    }
}

conform ExactSizeIterator for AllocatingIter {
    method &self.remaining_len(): usize {
        return self.end - self.next_value
    }
}

conform Iterator for AllocatingIter {
    type Item = usize

    method &+self.next(): usize? {
        if self.next_value >= self.end {
            return none
        }
        let current = self.next_value
        self.next_value = current + 1
        return current
    }
}

func run(source: AllocatingCollection): void {
    let values = Vec [...move source]
    drop values
    return
}
"#;
    let source = project.write_source("sequence_spread_allocation_lsp.nct", source_text);
    let uri = file_uri(&source);
    let run_offset = source_text.rfind("run(source").unwrap();
    let spread_offset = source_text.rfind("move source").unwrap() + "move ".len();
    let output = nocter_lsp(
        &distributed_home().join("nocter"),
        project.root(),
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({
                "jsonrpc":"2.0","method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":uri,"languageId":"nocter","version":1,"text":source_text}}
            }),
            json!({
                "jsonrpc":"2.0","id":2,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(source_text,run_offset)}
            }),
            json!({
                "jsonrpc":"2.0","id":3,"method":"textDocument/hover",
                "params":{"textDocument":{"uri":uri},"position":typed_literal_lsp_position(source_text,spread_offset)}
            }),
            json!({"jsonrpc":"2.0","id":4,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let frames = read_frames(&output.stdout);
    let response = |id| {
        frames
            .iter()
            .find(|message| message["id"] == id)
            .unwrap_or_else(|| panic!("missing LSP response {id}: {frames:#?}"))
    };
    let run_hover = response(2)["result"]["contents"]["value"]
        .as_str()
        .expect("expected run hover");
    assert!(!run_hover.contains("Allocation effect"), "{run_hover}");
    assert!(!run_hover.contains("alloc func run"), "{run_hover}");
    let spread_hover = response(3)["result"]["contents"]["value"]
        .as_str()
        .expect("expected spread hover");
    assert!(
        !spread_hover.contains("Allocation effect"),
        "{spread_hover}"
    );
}

#[test]
fn distributed_std_copy_spread_keeps_its_readonly_loan_until_literal_call() {
    let project = TempProject::new("distributed-home-sequence-spread-borrow-check");
    let source = project.write_source(
        "sequence_spread_borrow.nct",
        r#"use std/vec.{Vec, clear}

func mutate(values: &+Vec<i32>): i32 {
    clear(values)
    return 9
}

func main(): i32 {
    var source = Vec [1, 2]
    let invalid_copy = Vec [...source, mutate(&+source)]
    let invalid_readonly = Vec [...&source, mutate(&+source)]
    let valid_after_call = Vec [...source].len() + mutate(&+source)
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0434]").count(), 2, "{stderr}");
    assert!(stderr.contains("source"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_sequence_spread_runs_without_materialization() {
    let project = TempProject::new("distributed-home-sequence-spread-run");
    let source = project.write_source(
        "sequence_spread_run.nct",
        r#"use std/vec.{Vec, into_iter}

func main(): i32 {
    let source = Vec [1, 2, 3]
    let other = Vec [7, 8]
    let copied = Vec [0, ...source, ...other, ...source, 9]
    if copied.len() != 10 || (&copied as &[i32])[0] != 0 || (&copied as &[i32])[4] != 7 || (&copied as &[i32])[8] != 3 {
        return 1
    }
    if source.len() != 3 || other.len() != 2 {
        return 2
    }

    let borrowed = Vec [String "borrowed"]
    let references: Vec<&String> = Vec [...&borrowed]
    if references.len() != 1 {
        return 3
    }

    let owned = Vec [String "a", String "b"]
    let owned_suffix = Vec [String "c"]
    let moved = Vec [String "z", ...move owned, ...move owned_suffix]
    if moved.len() != 4 {
        return 4
    }

    let direct_source = Vec [String "direct"]
    let direct_iterator = into_iter(move direct_source)
    let direct = Vec [...move direct_iterator]
    if direct.len() != 1 {
        return 5
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
fn sequence_spread_pack_length_is_cached_before_consumption() {
    let project = TempProject::new("distributed-home-sequence-spread-cached-length-run");
    let source = project.write_source(
        "index.nct",
        r#"use std/vec.Vec

struct Count {
    before: usize
    after: usize
}

construct Count {
    pub default literal [](...items: i32): Self {
        let before = items.len()
        for item in items {
            let copy = item
        }
        let after = items.len()
        return Count { before: before, after: after }
    }
}

func main(): i32 {
    let source = Vec [1, 2, 3]
    let count = Count [0, ...source, 4]
    if count.before != 5 || count.after != 5 {
        return 1
    }
    return 42
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
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
    if (&values as &[i32])[0] != 10 || (&values as &[i32])[1] != 20 || (&values as &[i32])[2] != 12 {
        return 2
    }

    let empty: Vec<i32> = Vec []
    if !empty.is_empty() || empty.capacity() != 0 {
        return 3
    }

    let text = String "hello"
    if (&text as &str) != "hello" || text.len() != 5 {
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
        if values.len() != 3 || (&text as &str) != "region" {
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
    if values.len() != 2 || (&values as &[i32])[0] != 20 || (&values as &[i32])[1] != 22 {
        return 1
    }
    if (&text as &str) != "explicit" {
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
        "index.nct",
        r#"use std/io.print

struct Token {
    label: &str
}

destruct Token(&+self) {
    print(self.label)!
    return
}

struct Sink {
    code: i32
}

construct Sink {
    pub default literal [](...items: Token): Self {
        return Sink { code: 0 }
    }
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
        "index.nct",
        r#"use std/vec.Vec

struct Exhausted {
    code: i32
}

construct Exhausted {
    pub default literal [](...items: i32): Self {
        let impossible: Vec<u8> = Vec.with_capacity(18446744073709551615)
        return Exhausted { code: 0 }
    }
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

#[test]
fn readonly_sequence_spread_preserves_element_region_provenance() {
    let project = TempProject::new("distributed-home-sequence-spread-region-escape-check");
    let source = project.write_source(
        "sequence_spread_region_escape.nct",
        r#"use std/mem.page_allocator
use std/vec.Vec

func leak(): Vec<&String> {
    let result_arena = page_allocator()
    let temporary_arena = page_allocator()
    region temporary using temporary_arena {
        let source = Vec [String "temporary"]
        return Vec [...&source] using result_arena
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0436]"), "{stderr}");
    assert!(stderr.contains("region `temporary`"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literal_region_release_unmaps_literal_owned_storage() {
    let project = TempProject::new("distributed-home-typed-literal-region-unmap-run");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);
    let vec_module = home.join("std/vec/index.nct");
    let vec_source = fs::read_to_string(&vec_module).unwrap();
    fs::write(
        &vec_module,
        format!(
            "{vec_source}\n\npub func storage_address<T>(values: &Vec<T>): usize {{\n    return addr(values.storage.ptr)\n}}\n"
        ),
    )
    .unwrap();
    fs::create_dir_all(home.join("std/literal_region_probe")).unwrap();
    fs::write(
        home.join("std/literal_region_probe/index.nct"),
        r#"use std/internal/os.syscall3

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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn typed_literal_failure_restores_region_allocator_before_outer_drop() {
    let project = TempProject::new("distributed-home-literal-override-drop-context-run");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);
    fs::create_dir_all(home.join("std/allocation_context_probe")).unwrap();
    fs::write(
        home.join("std/allocation_context_probe/index.nct"),
        r#"use std/mem.current_allocator_kind
use std/process.exit_raw

pub func assert_region_allocator(): void {
    if current_allocator_kind() != 1 {
        exit_raw(91)
    }
    return
}
"#,
    )
    .unwrap();
    let source = project.write_source(
        "index.nct",
        r#"use std/allocation_context_probe.assert_region_allocator
use std/mem.page_allocator

struct Numbers { count: usize }
struct Token { value: i32 }

destruct Token(&+self) {
    assert_region_allocator()
    return
}

construct Numbers {
    pub default literal [](...items: i32): Self {
        return Numbers { count: items.len() }
    }
}

func next(): i32! {
    return error.new("test.failure", "expected failure")
}

func operation(): i32! {
    let region_allocator = page_allocator()
    let root_allocator = page_allocator()
    region temporary using region_allocator {
        let token = Token { value: 1 }
        let values = Numbers [next()?] using root_allocator
    }
    return 0
}

func main(): i32 {
    return operation() catch expected_failure {
        return 42
    }
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn sequence_spread_early_literal_exit_drops_owned_suffix_once() {
    let project = TempProject::new("distributed-home-sequence-spread-drop-run");
    let source = project.write_source(
        "index.nct",
        r#"use std/io.print
use std/vec.Vec

struct Token {
    label: &str
}

destruct Token(&+self) {
    print(self.label)!
    return
}

struct Sink {
    code: i32
}

construct Sink {
    pub default literal [](...items: Token): Self {
        return Sink { code: 0 }
    }
}

func main(): i32 {
    let source = Vec [Token { label: "A" }, Token { label: "B" }, Token { label: "C" }]
    let sink = Sink [...move source]
    return 42
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stderr:\n{}",
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"CBA");
    assert!(output.stderr.is_empty(), "expected empty stderr");
}
