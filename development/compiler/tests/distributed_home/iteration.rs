use super::*;

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
