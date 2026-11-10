//! Complete source arrays through the common Machine and native process.
use super::{TempPackage, compile, execute_native_status};

#[test]
fn private_computed_array_headers_reach_native_without_runtime_array_values() {
    for (name, source) in [
        (
            "array-header-body-types",
            "struct Data { values: [i32; width()] }\nconst BASE: usize = 40\nconst func width(): usize { let two: usize = 2\nlet pair = (BASE, two)\npair.0 + pair.1 }\nfunc run(): i32 { if width() == 42 { 42 } else { 0 } }",
        ),
        (
            "array-header-empty-recursion",
            "copy struct Node { next: [Node; zero()] }\nconst func zero(): usize { 0 }\nfunc run(): i32 { 42 }",
        ),
        (
            "array-header-large-metadata",
            "copy struct Huge { values: [[error; 0]; 18446744073709551615] }\nfunc run(): i32 { 42 }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_array_computation_is_frozen_before_native_instance_closure() {
    for (name, source) in [
        (
            "array-const-selection",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/array-const-selection.nct"
            )),
        ),
        (
            "array-const-repetition",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/array-const-repetition.nct"
            )),
        ),
        (
            "array-const-scoped-edit",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/array-const-scoped-edit.nct"
            )),
        ),
        (
            "array-const-indexed-write",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/array-const-indexed-write.nct"
            )),
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_listed_arrays_execute_with_saved_checked_element_addresses() {
    for (name, source) in [
        (
            "array-read",
            "func run(): i32 { let values = [20, 22]\nvalues[0] + values[1] }",
        ),
        ("array-computed-copy", "func run(): i32 { [7, 42][1] }"),
        (
            "array-scoped-edit",
            "func change(edit values: [i32; 2]): void { var index: usize = 0\nedit selected = values[index]\nindex = 1\nselected = 40\nselected += 2 }\nfunc run(): i32 { var values = [1, 7]\nchange(values)\nvalues[0] }",
        ),
        (
            "array-snapshot",
            "func run(): i32 { var values = [0, 7]\nlet snapshot = values\nvalues[0] = 40\nvalues[0] += 2\nvalues[0] + snapshot[0] }",
        ),
        (
            "array-take-return",
            "func pass(take values: [i32; 6]): [i32; 6] { values^ }\nfunc build(): [i32; 6] { pass([20, 1, 2, 3, 4, 22]) }\nfunc run(): i32 { let values = build()\nvalues[0] + values[5] }",
        ),
        (
            "array-nested-record",
            "struct Row { values: [i32; 2] }\nfunc run(): i32 { var rows = [Row { values: [0, 0] }]\nrows[0] = Row { values: [40, 7] }\nrows[0].values[0] += 2\nrows[0].values[0] }",
        ),
        (
            "array-loop",
            "func run(): i32 { var values = [0, 0]\nvar index: usize = 0\nwhile index < 2 { values[index] = 20\nvalues[index] += 1\nindex += 1 }\nvalues[0] + values[1] }",
        ),
        (
            "array-rhs-first",
            "func produce(edit log: i32): i32 { log = log * 10 + 1\n42 }\nfunc position(edit log: i32): usize { log = log * 10 + 2\n0 }\nfunc run(): i32 { var values = [0]\nvar log = 0\nvalues[position(log)] = produce(log)\nvalues[0] + log - 12 }",
        ),
        (
            "array-zero-size-index-effect",
            "func position(edit count: i32): usize { count += 1\n1 }\nfunc run(): i32 { var count = 40\nvar values = [(), ()]\nvalues[position(count)] = ()\nif [(), ()][position(count)] == () { count } else { 0 } }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_frozen_arrays_serialize_listed_and_sparse_repeated_storage() {
    for (name, source) in [
        (
            "array-frozen-listed",
            "const DATA: [i32; 2] = [20, 22]\nfunc run(): i32 { var own = DATA\nown[0] = 0\nDATA[0] + DATA[1] + own[0] }",
        ),
        (
            "array-frozen-repeated",
            "const func build(): [i32; 3] { var values = [1; 3]\nvalues[1] = 40\nvalues^ }\nstatic BASE: [i32; 3] = build()\nconst DATA: [i32; 3] = build()\nfunc run(): i32 { var own = DATA\nown[1] = 0\nBASE[1] + DATA[0] + DATA[2] + own[1] }",
        ),
        (
            "array-zero-byte-max",
            "const DATA: [void; 18446744073709551615] = [(); 18446744073709551615]\nfunc run(): i32 { if DATA[18446744073709551614] == () { 42 } else { 0 } }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_array_bounds_trap_even_when_no_payload_is_read_or_written() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;
    for (name, source) in [
        (
            "array-empty-bounds",
            "func run(): i32 { let values: [i32; 0] = []\nlet _ = values[0]\n42 }",
        ),
        (
            "array-empty-write-bounds",
            "func run(): i32 { var values: [i32; 0] = []\nvalues[0] = 42\n0 }",
        ),
        (
            "array-empty-recursive-bounds",
            "struct Node { next: [Node; 0] }\nfunc run(): i32 { let values: [Node; 0] = []\nread item = values[0]\n42 }",
        ),
        (
            "array-read-bounds",
            "func run(): i32 { let values = [1, 2]\nlet _ = values[2]\n42 }",
        ),
        (
            "array-write-bounds",
            "func run(): i32 { var values = [1, 2]\nvalues[2] = 42\n0 }",
        ),
        (
            "array-unit-copy-bounds",
            "func run(): i32 { let _ = [(), ()][2]\n42 }",
        ),
        (
            "array-unit-alias-bounds",
            "func run(): i32 { let values = [(), ()]\nread selected = values[2]\n42 }",
        ),
        (
            "array-unit-write-bounds",
            "func run(): i32 { var values = [(), ()]\nvalues[2] = ()\n42 }",
        ),
        (
            "array-maximum-index-bounds",
            "func run(): i32 { let values = [1]\nlet index: usize = 18446744073709551615\nlet _ = values[index]\n42 }",
        ),
    ] {
        let package = TempPackage::new();
        let executable = package.0.join(name);
        std::fs::write(&executable, compile(source).bytes()).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let status = Command::new(&executable)
            .current_dir(&package.0)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(5), "{name}: {status:?}");
    }
}

#[test]
fn private_repetition_constructs_once_and_fills_without_generated_element_lists() {
    for (name, source) in [
        (
            "array-repeat-snapshot",
            "func run(): i32 { var initial = 20\nvar values = [initial; 2]\ninitial = 0\nvalues[0] += 2\nvalues[0] + values[1] + initial }",
        ),
        (
            "array-repeat-one-noncopy",
            "struct Item { value: i32 }\nfunc run(): i32 { let values = [Item { value: 42 }; 1]\nvalues[0].value }",
        ),
        (
            "array-repeat-seed-effect",
            "func build(edit calls: i32): i32 { calls += 1\n20 }\nfunc run(): i32 { var calls = 0\nlet values = [build(calls); 3]\nvalues[0] + values[2] + calls + 1 }",
        ),
        (
            "array-repeat-nested",
            "func run(): i32 { var values = [[20, 2]; 3]\nvalues[1][0] += 20\nvalues[1][0] + values[1][1] }",
        ),
        (
            "array-repeat-copy-record",
            "copy struct Item { flag: bool\nvalue: i32 }\nfunc run(): i32 { let values = [Item { flag: true, value: 42 }; 5]\nif values[4].flag { values[4].value } else { 0 } }",
        ),
        (
            "array-repeat-wide-seed",
            "func run(): i32 { let values = [[1, 2, 3, 4, 5, 42]; 3]\nvalues[2][5] }",
        ),
        (
            "array-repeat-large",
            "func run(): i32 { var values = [40; 4096]\nvalues[4095] += 2\nvalues[4095] }",
        ),
        (
            "array-repeat-max-zero-byte",
            "func build(edit calls: i32): void { calls += 1 }\nfunc run(): i32 { var calls = 40\nlet values = [build(calls); 18446744073709551615]\nlet _ = values[18446744073709551614]\ncalls + 1 }",
        ),
        (
            "array-repeat-interrupted-seed",
            "func run(): i32 { let values = [if true { return 42 } else { 0 }; 4096]\nvalues[0] }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_empty_arrays_have_no_element_geometry_or_payload_dependency() {
    for (name, source) in [
        (
            "array-empty-call",
            "func pass(take values: [i32; 0]): [i32; 0] { values^ }\nfunc run(): i32 { let values: [i32; 0] = []\nlet _ = pass(values)\n42 }",
        ),
        (
            "array-empty-recursion",
            "struct Node { next: [Node; 0] }\nfunc build(): Node { Node { next: [] } }\nfunc run(): i32 { let node = build()\nlet _ = node.next\n42 }",
        ),
        (
            "array-empty-absent-payload",
            "struct Node { next: [Node; 0] }\nfunc build(): [Node; 0] { [] }\nfunc run(): i32 { let values = build()\n42 }",
        ),
        (
            "array-empty-nested-max",
            "struct Node { next: [Node; 0] }\nfunc run(): i32 { let nested: [[Node; 0]; 18446744073709551615] = [[]; 18446744073709551615]\nlet _ = nested[18446744073709551614]\n42 }",
        ),
        (
            "array-empty-seed-effect",
            "struct Item { value: i32 }\nfunc build(edit calls: i32): Item { calls += 1\nItem { value: 7 } }\nfunc run(): i32 { var calls = 41\nlet values = [build(calls); 0]\ncalls }",
        ),
        (
            "array-empty-frozen",
            "struct Node { next: [Node; 0] }\nconst EMPTY: [Node; 0] = []\nstatic BASE: [Node; 0] = []\nfunc inspect(values: [Node; 0]): i32 { 21 }\nfunc run(): i32 { inspect(EMPTY) + inspect(BASE) }",
        ),
        (
            "array-empty-edit",
            "func clear(edit values: [i32; 0]): i32 { values = []\n42 }\nfunc run(): i32 { var values: [i32; 0] = []\nclear(values) }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_complete_array_cleanup_can_reach_image_lowering() {
    // No source error-construction path is qualified here. Keep this evidence at image lowering;
    // executing the divergent producer would not validate element destruction.
    let image = compile(
        "struct Item { reason: error }\nfunc make(): [Item; 3] { loop {} }\nfunc run(): void { let values = make() }",
    );
    assert!(!image.bytes().is_empty());
    let package = TempPackage::new();
    execute_native_status(
        &compile("func build(): [error; 0] { [] }\nfunc run(): i32 { let values = build()\n42 }"),
        &package.0,
        "empty-error-array",
        42,
    );
}

#[test]
fn private_structural_array_regions_keep_disjoint_access_and_index_effects() {
    for (name, source) in [
        (
            "array-disjoint-inputs",
            "func add(edit left: i32, edit right: i32): void { left += 1\nright += 1 }\nfunc run(): i32 { var values = [20, 18]\nvar count = 0\nwhile count < 2 { add(values[0], values[1])\ncount += 1 }\nvalues[0] + values[1] }",
        ),
        (
            "array-unknown-disjoint-fields",
            "struct Row { left: i32\nright: i32 }\nfunc add(edit left: i32, edit right: i32): void { left += 1\nright += 1 }\nfunc change(edit rows: [Row; 2], i: usize, j: usize): void { add(rows[i].left, rows[j].right) }\nfunc run(): i32 { var rows = [Row { left: 20, right: 0 }, Row { left: 0, right: 20 }]\nchange(rows, 0, 1)\nrows[0].left + rows[1].right }",
        ),
        (
            "array-index-effects-singleton",
            "func work(flag: bool): i32 { var calls = 0\nvar values = [20, 20]\nedit left = values[if flag { calls += 1\n0 } else { calls += 10\n0 }]\nedit right = values[1]\nleft += 1\nright += 1\nleft + right + calls }\nfunc run(): i32 { work(false) - 10 }",
        ),
        (
            "array-disjoint-saved-fields",
            "struct Row { left: i32\nright: i32 }\nfunc run(): i32 { var rows = [Row { left: 10, right: 0 }, Row { left: 0, right: 0 }]\nvar index: usize = 0\nedit first = rows[index].left\nindex = 1\nedit second = rows[index].right\nfirst += 32\nsecond += 0\nfirst + second }",
        ),
        (
            "array-disjoint-preparation-effects",
            "func add(edit value: i32, delta: i32): void { value += delta }\nfunc next(edit value: i32): i32 { value += 1\n1 }\nfunc run(): i32 { var values = [20, 20]\nadd(values[0], next(values[1]))\nvalues[0] + values[1] }",
        ),
        (
            "array-zero-byte-disjoint-inputs",
            "func assign(edit left: void, edit right: void): i32 { left = ()\nright = ()\n42 }\nfunc run(): i32 { var values = [(), ()]\nassign(values[0], values[1]) }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}
