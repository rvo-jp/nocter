use super::*;

#[test]
fn distributed_std_vec_access_and_mutation_surface_passes_check() {
    let project = TempProject::new("distributed-home-vec-access-check");
    let source = project.write_source(
        "vec_access_shape.nct",
        r#"use std/vec.{Vec, insert, remove, try_insert}

func mutate(values: &+Vec<i32>): void! {
    let read: &i32 = values.get(0) otherwise { return }
    let write: &+i32 = values.get_mut(0) otherwise { return }
    try_insert(values, 0, 1)?
    insert(values, 1, 2)
    let removed: i32 = remove(values, 0) otherwise { return }
    return
}

func methods(values: &+Vec<i32>): void! {
    let read: &i32 = values.get(0) otherwise { return }
    let write: &+i32 = values.get_mut(0) otherwise { return }
    values.try_insert(0, 1)?
    values.insert(1, 2)
    let removed: i32 = values.remove(0) otherwise { return }
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[test]
fn distributed_std_vec_get_borrows_keep_the_source_loan_active() {
    let project = TempProject::new("distributed-home-vec-get-borrow-check");
    let source = project.write_source(
        "vec_get_borrow.nct",
        r#"use std/vec.Vec

copy struct Cell {
    value: i32
}

func read(cell: &Cell): i32 {
    return cell.value
}

func read_mut(cell: &+Cell): i32 {
    return cell.value
}

func invalid_readonly(): i32 {
    var values = Vec [Cell { value: 42 }]
    let cell = values.get(0) otherwise { return 0 }
    values.push(Cell { value: 1 })
    return read(cell)
}

func invalid_readwrite(): i32 {
    var values = Vec [Cell { value: 42 }]
    let cell = values.get_mut(0) otherwise { return 0 }
    values.push(Cell { value: 1 })
    return read_mut(cell)
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.matches("error[E0434]").count(), 2, "{stderr}");
    assert!(
        stderr.contains("values") && stderr.contains("cell"),
        "{stderr}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_get_and_get_mut_observe_bounds_and_mutation() {
    let project = TempProject::new("distributed-home-vec-get-run");
    let source = project.write_source(
        "vec_get_run.nct",
        r#"use std/vec.Vec

copy struct Cell {
    value: i32
}

func read(cell: &Cell): i32 {
    return cell.value
}

func write(cell: &+Cell, value: i32): void {
    cell.value = value
    return
}

func missing(values: &Vec<Cell>): i32 {
    let unexpected = values.get(2) otherwise { return 0 }
    return 10
}

func missing_mut(values: &+Vec<Cell>): i32 {
    let unexpected = values.get_mut(2) otherwise { return 0 }
    return 10
}

func main(): i32 {
    var values = Vec [Cell { value: 2 }, Cell { value: 2 }]
    let first = values.get(0) otherwise { return 1 }
    let first_value: i32 = read(first)
    let second = values.get_mut(1) otherwise { return 2 }
    write(second, 40)
    let changed = values.get(1) otherwise { return 3 }
    return first_value + read(changed) + missing(&values) + missing_mut(&+values)
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
fn distributed_std_vec_insert_and_remove_preserve_move_only_order() {
    let project = TempProject::new("distributed-home-vec-insert-remove-run");
    let source = project.write_source(
        "vec_insert_remove_run.nct",
        r#"use std/vec.Vec

func main(): i32! {
    var values = Vec [String "A", String "C"]
    values.insert(1, String "B")
    if values.len() != 3 {
        return 1
    }
    let first = values.get(0) otherwise { return 2 }
    let middle = values.get(1) otherwise { return 2 }
    let last = values.get(2) otherwise { return 2 }
    if read(first) != "A" || read(middle) != "B" || read(last) != "C" {
        return 2
    }

    let removed = values.remove(1) otherwise { return 3 }
    if (&removed as &str) != "B" {
        return 4
    }
    let remaining_first = values.get(0) otherwise { return 5 }
    let remaining_last = values.get(1) otherwise { return 5 }
    if values.len() != 2 || read(remaining_first) != "A" || read(remaining_last) != "C" {
        return 5
    }

    values.try_insert(2, String "D")?
    let inserted = values.get(2) otherwise { return 6 }
    if read(inserted) != "D" {
        return 6
    }
    return 42
}

func read(value: &String): &str {
    return (value as &str)
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
fn distributed_std_vec_preserves_nonlegacy_integer_widths() {
    let project = TempProject::new("distributed-home-vec-integer-widths-run");
    let source = project.write_source(
        "vec_integer_widths_run.nct",
        r#"use std/vec.Vec

func main(): i32 {
    var unsigned: Vec<u16> = Vec [10 as u16, 20 as u16]
    unsigned.push(12)
    let first: u16 = unsigned.remove(0) otherwise { return 1 }
    let second: u16 = unsigned.remove(0) otherwise { return 2 }
    let third: u16 = unsigned.remove(0) otherwise { return 3 }
    if first != 10 || second != 20 || third != 12 {
        return 4
    }

    var signed: Vec<i16> = Vec [-3 as i16, -2 as i16]
    let signed_view = &+signed as &+[i16]
    signed_view[0] += 1
    if signed_view[0] != -2 {
        return 5
    }
    let negative: i16 = signed.remove(0) otherwise { return 5 }
    if negative != -2 {
        return 6
    }
    let iterated: Vec<i16> = Vec [20 as i16, 22 as i16]
    var total: i16 = 0
    for item in move iterated {
        total += item
    }
    if total != 42 {
        return 7
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_try_insert_bounds_failure_preserves_the_vector() {
    let project = TempProject::new("distributed-home-vec-insert-bounds-run");
    let source = project.write_source(
        "vec_insert_bounds_run.nct",
        r#"use std/vec.Vec

func probe(values: &+Vec<i32>): i32 {
    values.try_insert(3, 7) catch insertion_error {
        return 42
    }
    return 3
}

func main(): i32 {
    var values = Vec [20, 22]
    let result = probe(&+values)
    if result != 42 {
        return result
    }
    if values.len() != 2 || values.capacity() != 2 {
        return 1
    }
    if (&values as &[i32])[0] != 20 || (&values as &[i32])[1] != 22 {
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_try_insert_growth_failure_is_state_atomic() {
    let project = TempProject::new("distributed-home-vec-insert-growth-failure-run");
    let home = project.root().join(".nocter");
    copy_tree(&distributed_home(), &home);

    let mem_module = home.join("std/mem/index.nct");
    let mem_source = fs::read_to_string(&mem_module).unwrap();
    let original = r#"pub(nocter) func try_grow_owned(buffer: &+RawBuffer, new_size: usize): void! {
    var allocator = TryAllocator {
        state: buffer.allocator_state,
        kind: buffer.allocator_kind,
    }
    if buffer.allocator_kind == 2 {
        allocator.state = current_allocator_state()
        allocator.kind = current_allocator_kind()
    }
    try_grow(&+allocator, buffer, new_size)?
    return
}"#;
    let failing = r#"pub(nocter) func try_grow_owned(buffer: &+RawBuffer, new_size: usize): void! {
    return Error.new("test.out_of_memory", "deterministic growth failure")
}"#;
    assert!(mem_source.contains(original));
    fs::write(&mem_module, mem_source.replace(original, failing)).unwrap();

    let vec_module = home.join("std/vec/index.nct");
    let vec_source = fs::read_to_string(&vec_module).unwrap();
    fs::write(
        &vec_module,
        format!(
            "{vec_source}\n\npub func storage_address<T>(values: &Vec<T>): usize {{\n    return addr(values.storage.ptr)\n}}\n"
        ),
    )
    .unwrap();

    let source = project.write_source(
        "vec_insert_growth_failure_run.nct",
        r#"use std/vec.{Vec, storage_address}

func attempt(values: &+Vec<i32>): i32 {
    values.try_insert(1, 7) catch allocation_error {
        return 0
    }
    return 1
}

func main(): i32 {
    var values: Vec<i32> = Vec.with_capacity(2)
    values.push(20)
    values.push(22)
    let before = storage_address(&values)
    if attempt(&+values) != 0 {
        return 1
    }
    let after = storage_address(&values)
    if before != after || values.len() != 2 || values.capacity() != 2 {
        return 2
    }
    if (&values as &[i32])[0] != 20 || (&values as &[i32])[1] != 22 {
        return 3
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
