use super::*;

#[test]
fn distributed_std_collection_for_surface_passes_check() {
    let project = TempProject::new("distributed-home-collection-for-check");
    let source = project.write_source(
        "collection_for_shape.nct",
        r#"use std/vec.Vec

func read(value: &i32): i32 {
    return 0
}

func main(): i32 {
    let readonly = Vec [1, 2, 3]
    for item in &readonly {
        read(item)
    }

    let owned = Vec [4, 5, 6]
    for item in move owned {
        read(&item)
    }

    let source = Vec [7, 8, 9]
    let iterator = (move source).into_iter()
    for item in iterator {
        read(&item)
    }
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_collection_for_rejects_a_bare_collection() {
    let project = TempProject::new("distributed-home-collection-for-bare-collection");
    let source = project.write_source(
        "collection_for_bare_collection.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec [1, 2, 3]
    for item in values {
        return item
    }
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0448]"), "{stderr}");
    assert!(stderr.contains("explicit ownership mode"), "{stderr}");
}

#[test]
fn distributed_std_collection_for_keeps_the_readonly_source_loan_active() {
    let project = TempProject::new("distributed-home-collection-for-readonly-loan");
    let source = project.write_source(
        "collection_for_readonly_loan.nct",
        r#"use std/vec.Vec

func read(value: &i32): void {
    return
}

func main(): i32 {
    var values = Vec [1, 2, 3]
    for item in &values {
        values.push(4)
        read(item)
    }
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0434]"), "{stderr}");
    assert!(stderr.contains("values"), "{stderr}");
}

#[test]
fn distributed_std_collection_for_consumes_owned_sources_and_direct_iterators() {
    let project = TempProject::new("distributed-home-collection-for-consumes-sources");
    let source = project.write_source(
        "collection_for_consumes_sources.nct",
        r#"use std/vec.Vec

func owned(): void {
    let values = Vec [1, 2, 3]
    for item in move values {
        let copy = item
    }
    let after = values.len()
    return
}

func direct(): void {
    let values = Vec [1, 2, 3]
    let iterator = (move values).into_iter()
    for item in iterator {
        let copy = item
    }
    let after = iterator.remaining()
    return
}

func main(): i32 {
    owned()
    direct()
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0385]").count(), 2, "{stderr}");
    assert!(
        stderr.contains("values") && stderr.contains("iterator"),
        "{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_collection_for_readonly_owned_and_direct_iteration_runs() {
    let project = TempProject::new("distributed-home-collection-for-run");
    let source = project.write_source(
        "collection_for_run.nct",
        r#"use std/vec.Vec

copy struct Value {
    number: i32
}

func read(value: &Value): i32 {
    return value.number
}

func main(): i32 {
    let readonly = Vec [
        Value { number: 4 },
        Value { number: 5 },
        Value { number: 6 },
    ]
    var total: i32 = 0
    for item in &readonly {
        total = total + read(item)
    }

    let owned = Vec [7, 8, 9]
    for item in move owned {
        total = total + item
    }

    let source = Vec [1, 2, 0]
    let iterator = (move source).into_iter()
    for item in iterator {
        total = total + item
    }
    return total
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

#[test]
fn distributed_std_readonly_iterator_surface_passes_check() {
    let project = TempProject::new("distributed-home-readonly-iterator-check");
    let source = project.write_source(
        "readonly_iterator_shape.nct",
        r#"use std/iter.{ViewIter, from_view, next, remaining}
use std/vec.Vec

func view_shape(values: &[i32]): usize {
    var first: ViewIter<i32> = ViewIter.from_view(values)
    let item: &i32 = first.next() otherwise { return 0 }
    var second = from_view(values)
    let other: &i32 = next(&+second) otherwise { return 0 }
    return remaining(&first) + remaining(&second)
}

func collection_shape(values: &Vec<i32>, text: &String): usize {
    let value_iterator: ViewIter<i32> = values.iter()
    let byte_iterator: ViewIter<u8> = text.bytes_iter()
    return value_iterator.remaining() + byte_iterator.remaining()
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
fn distributed_std_iterator_keeps_the_source_borrow_active_until_last_use() {
    let project = TempProject::new("distributed-home-readonly-iterator-borrow");
    let source = project.write_source(
        "readonly_iterator_borrow.nct",
        r#"use std/vec.Vec

func main(): i32 {
    var values: Vec<i32> = Vec [1, 2, 3]
    var iterator = values.iter()
    values.push(4)
    let left = iterator.remaining()
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
    assert!(stderr.contains("error[E0434]"), "{stderr}");
    assert!(
        stderr.contains("values") && stderr.contains("iterator"),
        "{stderr}"
    );
}

#[test]
fn distributed_std_iterator_element_borrow_keeps_the_source_loan_active() {
    let project = TempProject::new("distributed-home-readonly-iterator-element-borrow");
    let source = project.write_source(
        "readonly_iterator_element_borrow.nct",
        r#"use std/vec.Vec

func read(value: &i32): void {
    return
}

func main(): i32 {
    var values: Vec<i32> = Vec [1, 2, 3]
    var iterator = values.iter()
    let item = iterator.next() otherwise { return 1 }
    drop iterator
    values.push(4)
    read(item)
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0434]"), "{stderr}");
    assert!(
        stderr.contains("values") && stderr.contains("item"),
        "{stderr}"
    );
}

#[test]
fn distributed_std_owned_iterator_surface_passes_check() {
    let project = TempProject::new("distributed-home-owned-iterator-check");
    let source = project.write_source(
        "owned_iterator_shape.nct",
        r#"use std/vec.{Vec, into_iter}
use std/vec_into_iter.{VecIntoIter, next, remaining}

func consume(values: Vec<i32>): usize {
    var first: VecIntoIter<i32> = into_iter(move values)
    let item: i32 = next(&+first) otherwise { return 0 }
    var second = Vec [1, 2, 3].into_iter()
    let other: i32 = second.next() otherwise { return 0 }
    return remaining(&first) + second.remaining()
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_owned_iterator_consumes_the_source_vec() {
    let project = TempProject::new("distributed-home-owned-iterator-consumes-source");
    let source = project.write_source(
        "owned_iterator_consumes_source.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec [1, 2, 3]
    let iterator = (move values).into_iter()
    return values.view()[0]
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
    assert!(stderr.contains("error[E0385]"), "{stderr}");
    assert!(
        stderr.contains("values") && stderr.contains("moved"),
        "{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_owned_vec_iteration_runs_in_source_order() {
    let project = TempProject::new("distributed-home-owned-iterator-run");
    let source = project.write_source(
        "owned_iterator_run.nct",
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec [4, 11, 27]
    var iterator = (move values).into_iter()
    var total: i32 = 0
    loop {
        let item = iterator.next() otherwise { break }
        total = total + item
    }
    if iterator.remaining() != 0 {
        return 1
    }
    return total
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
fn distributed_std_owned_iterator_drops_only_the_remaining_suffix_in_reverse() {
    let project = TempProject::new("distributed-home-owned-iterator-drop-run");
    let source = project.write_source(
        "owned_iterator_drop_run.nct",
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

func main(): i32 {
    let values = Vec [
        Token { label: "A" },
        Token { label: "B" },
        Token { label: "C" },
        Token { label: "D" },
    ]
    var iterator = (move values).into_iter()
    let first = iterator.next() otherwise { return 1 }
    drop first
    drop iterator
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
    assert_eq!(output.stdout, b"ADCB");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_readonly_vec_and_string_iteration_runs() {
    let project = TempProject::new("distributed-home-readonly-iterator-run");
    let source = project.write_source(
        "readonly_iterator_run.nct",
        r#"use std/ptr.{addr, from_ref}
use std/vec.Vec

copy struct Value {
    number: i32
}

func read(value: &Value): i32 {
    return value.number
}

func main(): i32! {
    var values: Vec<Value> = Vec.empty()
    values.push(Value { number: 4 })
    values.push(Value { number: 11 })
    values.push(Value { number: 27 })

    var iterator = values.iter()
    var total: i32 = 0
    loop {
        let item = iterator.next() otherwise { break }
        total = total + read(item)
    }
    if iterator.remaining() != 0 {
        return 1
    }

    let text = String "AZ"
    var bytes = text.bytes_iter()
    let first = bytes.next() otherwise { return 2 }
    let second = bytes.next() otherwise { return 3 }
    let first_address = addr(from_ref(first))
    let second_address = addr(from_ref(second))
    if second_address != first_address + 1 {
        return 5
    }
    let encoding = text.bytes()
    if encoding[0] != 65 {
        return 6
    }
    if encoding[1] != 90 {
        return 7
    }
    let unexpected = bytes.next() otherwise { return total }
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
fn distributed_std_vec_i32_iteration_preserves_storage_and_source_order() {
    let project = TempProject::new("distributed-home-readonly-i32-iterator-run");
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
    let source = project.write_source(
        "readonly_i32_iterator_run.nct",
        r#"use std/ptr.{addr, from_ref}
use std/vec.{Vec, storage_address}

func main(): i32 {
    let values: Vec<i32> = Vec [4, 11, 27]
    let before = storage_address(&values)
    let observed = values.view()
    var iterator = values.iter()
    var index: usize = 0
    loop {
        let item = iterator.next() otherwise { break }
        if index >= observed.len() {
            return 1
        }
        let item_pointer = from_ref(item)
        let item_address = addr(item_pointer)
        let expected_address = before + index * 4
        if item_address != expected_address {
            return 2
        }
        index = index + 1
    }
    if index != 3 || iterator.remaining() != 0 {
        return 3
    }
    if storage_address(&values) != before {
        return 4
    }
    if observed[0] != 4 || observed[1] != 11 || observed[2] != 27 {
        return 5
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
