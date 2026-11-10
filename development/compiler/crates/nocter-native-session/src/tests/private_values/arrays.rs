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
