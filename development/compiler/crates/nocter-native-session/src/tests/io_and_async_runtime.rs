use super::*;

#[test]
fn standard_io_descriptor_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"output\", module: \"./io\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["io"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard I/O tests failed native compilation")
    };
    assert_eq!(cases.len(), 5);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn public_writer_line_adapter_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source("index.nct", IO_WRITER_CONTRACT_TEST_SOURCE);
    package_root.source("implementation.nct", IO_WRITER_IMPLEMENTATION_TEST_SOURCE);
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:io-writer-tests");
    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
        vec![ModuleIdentity::new(package, Vec::<&str>::new())],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("public BlockingWriter line tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_num_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"numeric\", module: \"./num\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["num"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard numeric tests failed native compilation")
    };
    assert_eq!(cases.len(), 17);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_checksum_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"checksum\", module: \"./checksum\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["checksum"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard checksum tests failed native compilation")
    };
    assert_eq!(cases.len(), 3);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_time_value_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"time\", module: \"./time\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["time"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard time value tests failed native compilation")
    };
    assert_eq!(cases.len(), 23);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_async_sleep_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/time\n\
         \n\
         async func main(): i32 {\n\
             let duration = time.Duration.from_milliseconds(30)\n\
             let start = time.Instant.now()\n\
             await time.sleep(duration)\n\
             if start.elapsed() < duration { return 1 }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-sleep", 0);
}

#[test]
fn generic_async_io_defaults_cross_interface_dispatch_and_native_execution() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        include_str!("../../../../tests/fixtures/native/async_io_defaults.nct"),
    );
    execute_native_status(&image, &package_root.0, "async-io-defaults", 0);
}

#[test]
fn structured_async_join_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/task\n\
         use std/time\n\
         \n\
         async func number(value: i32): i32 { return value }\n\
         async func delayed_number(value: i32, milliseconds: u64): i32 {\n\
             await time.sleep(time.Duration.from_milliseconds(milliseconds))\n\
             return value\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let immediate = await task.join(number(20), number(22))\n\
             if immediate.0 + immediate.1 != 42 { return 1 }\n\
             let duration = time.Duration.from_milliseconds(35)\n\
             let start = time.Instant.now()\n\
             let delayed = task.join(\n\
                 delayed_number(20, 20),\n\
                 delayed_number(22, 35),\n\
             )\n\
             let delayed_values = await delayed\n\
             if delayed_values.0 + delayed_values.1 != 42 { return 2 }\n\
             if start.elapsed() < duration { return 3 }\n\
             let nested_duration = time.Duration.from_milliseconds(25)\n\
             let nested_start = time.Instant.now()\n\
             let nested = task.join(\n\
                 task.join(delayed_number(1, 5), delayed_number(2, 15)),\n\
                 delayed_number(3, 25),\n\
             )\n\
             let _ = await nested\n\
             if nested_start.elapsed() < nested_duration { return 4 }\n\
             let canceled = task.join(delayed_number(1, 35), delayed_number(2, 35))\n\
             drop canceled\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "structured-async-join", 0);
}

#[test]
fn structured_async_race_selects_one_winner_and_cancels_the_other() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/task\n\
         use std/task.Race\n\
         use std/time\n\
         use std/string.String\n\
         \n\
         async func number(value: i32): i32 { return value }\n\
         async func delayed_number(value: i32, milliseconds: u64): i32 {\n\
             await time.sleep(time.Duration.from_milliseconds(milliseconds))\n\
             return value\n\
         }\n\
         async func nested_value(): ((i32, i32), (i32, i32)) {\n\
             return ((5, 6), (7, 8))\n\
         }\n\
         async func delayed_text(value: String, milliseconds: u64): String {\n\
             await time.sleep(time.Duration.from_milliseconds(milliseconds))\n\
             return move value\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let immediate = await task.race(number(20), number(22))\n\
             match immediate {\n\
                 Race.first(value) { if value != 20 { return 1 } }\n\
                 Race.second(_) { return 2 }\n\
             }\n\
             let maximum = time.Duration.from_milliseconds(80)\n\
             let start = time.Instant.now()\n\
             let delayed = await task.race(\n\
                 delayed_number(20, 80),\n\
                 delayed_number(22, 10),\n\
             )\n\
             match delayed {\n\
                 Race.first(_) { return 3 }\n\
                 Race.second(value) { if value != 22 { return 4 } }\n\
             }\n\
             if !(start.elapsed() < maximum) { return 5 }\n\
             let partially_completed = task.join(\n\
                 task.join(number(1), number(2)),\n\
                 task.join(delayed_number(3, 80), delayed_number(4, 80)),\n\
             )\n\
             let nested_race = await task.race(move partially_completed, nested_value())\n\
             match nested_race {\n\
                 Race.first(_) { return 6 }\n\
                 Race.second(value) {\n\
                     if value.0.0 + value.0.1 + value.1.0 + value.1.1 != 26 { return 7 }\n\
                 }\n\
             }\n\
             let owned = await task.race(\n\
                 delayed_text(String.copy(\"lost\"), 80),\n\
                 delayed_text(String.copy(\"winner\"), 10),\n\
             )\n\
             match move owned {\n\
                 Race.first(_) { return 8 }\n\
                 Race.second(value) {\n\
                     if !(value == String.copy(\"winner\")) { return 9 }\n\
                 }\n\
             }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "structured-async-race", 0);
}

#[test]
fn structured_async_timeout_distinguishes_completion_from_elapsed_time() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/task\n\
         use std/task.Timeout\n\
         use std/time\n\
         \n\
         async func number(value: i32): i32 { return value }\n\
         async func delayed_number(value: i32, milliseconds: u64): i32 {\n\
             await time.sleep(time.Duration.from_milliseconds(milliseconds))\n\
             return value\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let immediate = await task.with_timeout(\n\
                 number(42),\n\
                 time.Duration.from_milliseconds(0),\n\
             )\n\
             match immediate {\n\
                 Timeout.completed(value) { if value != 42 { return 1 } }\n\
                 Timeout.elapsed { return 2 }\n\
             }\n\
             let elapsed = await task.with_timeout(\n\
                 delayed_number(42, 80),\n\
                 time.Duration.from_milliseconds(10),\n\
             )\n\
             match elapsed {\n\
                 Timeout.completed(_) { return 3 }\n\
                 Timeout.elapsed {}\n\
             }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(
        image.image(),
        &package_root.0,
        "structured-async-timeout",
        0,
    );
}

#[test]
fn dynamic_task_group_drives_runtime_sized_children_and_preserves_empty_state() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/task\n\
         use std/task.{TaskGroup, Timeout}\n\
         use std/time\n\
         \n\
         async func delayed_number(value: i32, milliseconds: u64): i32 {\n\
             await time.sleep(time.Duration.from_milliseconds(milliseconds))\n\
             return value\n\
         }\n\
         async func main(): i32 {\n\
             var empty: TaskGroup<i32> = TaskGroup.empty()\n\
             let absent = await empty.next()\n\
             let unexpected: i32 = absent otherwise {\n\
                 var group: TaskGroup<i32> = TaskGroup.empty()\n\
                 group.add(delayed_number(30, 30))\n\
                 group.add(delayed_number(10, 10))\n\
                 group.add(delayed_number(20, 20))\n\
                 if group.len() != 3 { return 1 }\n\
                 let first: i32 = await group.next() otherwise { return 2 }\n\
                 if first != 10 || group.len() != 2 { return 3 }\n\
                 let second: i32 = await group.next() otherwise { return 4 }\n\
                 if second != 20 || group.len() != 1 { return 5 }\n\
                 let third: i32 = await group.next() otherwise { return 6 }\n\
                 if third != 30 || !group.is_empty() { return 7 }\n\
                 var pairs: TaskGroup<(i32, i32)> = TaskGroup.empty()\n\
                 pairs.add(task.join(delayed_number(19, 5), delayed_number(23, 10)))\n\
                 let pair: (i32, i32) = await pairs.next() otherwise { return 8 }\n\
                 if pair.0 + pair.1 != 42 { return 9 }\n\
                 var retained: TaskGroup<i32> = TaskGroup.empty()\n\
                 retained.add(delayed_number(42, 30))\n\
                 let timed: Timeout<i32?> = await task.with_timeout(\n\
                     retained.next(),\n\
                     time.Duration.from_milliseconds(5),\n\
                 )\n\
                 match timed {\n\
                     Timeout.completed(_) { return 10 }\n\
                     Timeout.elapsed {}\n\
                 }\n\
                 if retained.len() != 1 { return 11 }\n\
                 let retained_value: i32 = await retained.next() otherwise { return 12 }\n\
                 if retained_value != 42 { return 13 }\n\
                 return 0\n\
             }\n\
             return unexpected + 8\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "dynamic-task-group", 0);
}

#[test]
fn suspended_child_can_read_parent_storage_without_parent_side_liveness() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/time\n\
         \n\
         struct Counter { value: i32 }\n\
         \n\
         async func read_after_delay(counter: &Counter): i32 {\n\
             await time.sleep(time.Duration.from_milliseconds(20))\n\
             return counter.value\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let counter = Counter { value: 42 }\n\
             let result = await read_after_delay(&counter)\n\
             if result == 42 { return 0 }\n\
             return 1\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "borrowed-parent-frame", 0);
}

#[test]
fn large_async_output_staging_preserves_the_consume_entry() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/time\n\
         use std/vec.Vec\n\
         \n\
         struct Payload {\n\
             first: Vec<u8>\n\
             second: Vec<u8>\n\
             third: Vec<u8>\n\
             fourth: Vec<u8>\n\
             fifth: Vec<u8>\n\
             sixth: Vec<u8>\n\
             seventh: Vec<u8>\n\
             eighth: Vec<u8>\n\
             ninth: Vec<u8>\n\
             tenth: Vec<u8>\n\
             eleventh: Vec<u8>\n\
             twelfth: Vec<u8>\n\
         }\n\
         \n\
         async func hold(payload: Payload): Payload {\n\
             await time.sleep(time.Duration.from_milliseconds(20))\n\
             return move payload\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let payload = Payload {\n\
                 first: Vec [u8.truncate(1)],\n\
                 second: Vec [u8.truncate(2)],\n\
                 third: Vec [u8.truncate(3)],\n\
                 fourth: Vec [u8.truncate(4)],\n\
                 fifth: Vec [u8.truncate(5)],\n\
                 sixth: Vec [u8.truncate(6)],\n\
                 seventh: Vec [u8.truncate(7)],\n\
                 eighth: Vec [u8.truncate(8)],\n\
                 ninth: Vec [u8.truncate(9)],\n\
                 tenth: Vec [u8.truncate(42)],\n\
                 eleventh: Vec [u8.truncate(11)],\n\
                 twelfth: Vec [u8.truncate(12)],\n\
             }\n\
             let result = await hold(move payload)\n\
             if result.tenth[0] == 42 { return 0 }\n\
             return 1\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "large-async-output", 0);
}

#[test]
fn large_fallible_async_output_preserves_the_failure_variant() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/vec.Vec\n\
         \n\
         struct Payload {\n\
             first: Vec<u8>\n\
             second: Vec<u8>\n\
             third: Vec<u8>\n\
             fourth: Vec<u8>\n\
             fifth: Vec<u8>\n\
             sixth: Vec<u8>\n\
             seventh: Vec<u8>\n\
             eighth: Vec<u8>\n\
             ninth: Vec<u8>\n\
             tenth: Vec<u8>\n\
             eleventh: Vec<u8>\n\
             twelfth: Vec<u8>\n\
         }\n\
         \n\
         struct Owner { payload: Payload }\n\
         struct Huge {\n\
             first: Payload\n\
             second: Payload\n\
             third: Payload\n\
             fourth: Payload\n\
             fifth: Payload\n\
             sixth: Payload\n\
             seventh: Payload\n\
             eighth: Payload\n\
         }\n\
         \n\
         instance Owner {\n\
             async method self.fail(): Huge! {\n\
                 return error.new(\"test.expected\", \"expected failure\")\n\
             }\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let payload = Payload {\n\
                 first: Vec [], second: Vec [], third: Vec [], fourth: Vec [],\n\
                 fifth: Vec [], sixth: Vec [], seventh: Vec [], eighth: Vec [],\n\
                 ninth: Vec [], tenth: Vec [], eleventh: Vec [], twelfth: Vec [],\n\
             }\n\
             let owner = Owner { payload: move payload }\n\
             let _payload = await owner.fail() catch failure {\n\
                 if failure.has_code(\"test.expected\") { return 0 }\n\
                 return 1\n\
             }\n\
             return 2\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(
        image.image(),
        &package_root.0,
        "large-fallible-async-output",
        0,
    );
}

#[test]
fn public_async_tcp_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/net\n\
         use std/task\n\
         use std/vec.Vec\n\
         \n\
         async func main(): i32! {\n\
             let address = net.SocketAddress.new(\n\
                 net.IpAddress.from_ipv4(net.Ipv4Address.loopback()),\n\
                 0,\n\
             )\n\
             var listener = await net.bind_tcp(address)?\n\
             let listening = listener.local_address()?\n\
             let connection = await task.join(\n\
                 net.connect_tcp(listening),\n\
                 listener.accept(),\n\
             )\n\
             let client_result = move connection.0\n\
             let accepted_result = move connection.1\n\
             var client = move client_result?\n\
             let accepted = move accepted_result?\n\
             var server = move accepted.0\n\
             await client.write(\"ping\".bytes())?\n\
             var buffer: Vec<u8> = Vec [\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
             ]\n\
             let count = await server.read(&+buffer)?\n\
             if count != 4 || buffer[0] != 112 || buffer[1] != 105\n\
                 || buffer[2] != 110 || buffer[3] != 103 { return 1 }\n\
             server.close()\n\
             if await client.read(&+buffer)? != 0 { return 2 }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-tcp", 0);
}

#[test]
fn standard_async_buffers_cross_generic_tcp_and_line_contracts() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_buffering_tcp.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-buffers", 0);
}

#[test]
fn standard_async_iterator_adapters_preserve_values_order_exhaustion_and_failure() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_iterator_adapters.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-iterator-adapters", 0);
}

#[test]
fn standard_async_streaming_producers_are_bounded_lazy_and_terminal() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_streaming_producers.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(
        image.image(),
        &package_root.0,
        "async-streaming-producers",
        0,
    );
}

#[test]
fn standard_async_copy_preserves_failure_cancellation_and_timeout_contracts() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_copy_contract.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-copy-contract", 0);
}

#[test]
fn standard_async_buffer_cancellation_preserves_reader_prefix_and_terminates_writer() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_buffering_cancellation.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(
        image.image(),
        &package_root.0,
        "async-buffer-cancellation",
        0,
    );
}

#[test]
fn public_async_udp_crosses_readiness_timeout_cancellation_and_datagram_boundaries() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/async_udp.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-udp", 0);
}

#[test]
fn provider_async_stream_policy_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(
        "\n#executable: { name: \"provider-async-stream\", module: \"./internal/net\" }\n",
    );
    let net_index_path = standard_root.join("internal/net/index.nct");
    let mut net_index_source = fs::read_to_string(&net_index_path).unwrap();
    net_index_source = net_index_source.replacen(
        "use /time.Duration\n",
        "use /time.Duration\nuse /vec.Vec\n",
        1,
    );
    net_index_source.push_str(PROVIDER_ASYNC_STREAM_TEST_MAIN);
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    overlay
        .insert_source(
            net_index_path,
            SourceOverride::new(net_index_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    let output = TempPackage::new();
    execute_native_status(image.image(), &output.0, "provider-async-stream", 0);
}

#[test]
fn provider_listener_policy_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source
        .push_str("\n#executable: { name: \"provider-listener\", module: \"./internal/net\" }\n");
    let net_index_path = standard_root.join("internal/net/index.nct");
    let mut net_index_source = fs::read_to_string(&net_index_path).unwrap();
    net_index_source = net_index_source.replacen(
        "use /time.Duration\n",
        "use /time.Duration\nuse /vec.Vec\n",
        1,
    );
    net_index_source.push_str(PROVIDER_LISTENER_POLICY_TEST_MAIN);
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    overlay
        .insert_source(
            net_index_path,
            SourceOverride::new(net_index_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    let output = TempPackage::new();
    execute_native_status(image.image(), &output.0, "provider-listener", 0);
}

#[test]
fn public_async_tcp_timeout_races_readiness_in_the_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/net\n\
         use std/time.Duration\n\
         use std/vec.Vec\n\
         \n\
         async func main(): i32! {\n\
             let address = net.SocketAddress.new(\n\
                 net.IpAddress.from_ipv4(net.Ipv4Address.loopback()),\n\
                 0,\n\
             )\n\
             let generous = Duration.from_seconds(1)\n\
             var listener = await net.bind_tcp(address)?\n\
             let listening = listener.local_address()?\n\
             var client = await net.connect_tcp_with_timeout(listening, generous)?\n\
             let accepted = await listener.accept_with_timeout(generous)?\n\
             var server = move accepted.0\n\
             await client.write_with_timeout(\"ok\".bytes(), generous)?\n\
             var buffer: Vec<u8> = Vec [u8.truncate(0), u8.truncate(0)]\n\
             let count = await server.read_with_timeout(&+buffer, generous)?\n\
             if count != 2 || buffer[0] != 111 || buffer[1] != 107 { return 1 }\n\
             var outgoing: Vec<u8> = Vec.with_capacity(4194304)\n\
             while outgoing.len() < 4194304 { outgoing.push(u8.truncate(120)) }\n\
             await client.write_with_timeout(\n\
                 &outgoing,\n\
                 Duration.from_milliseconds(2),\n\
             ) catch failure {\n\
                 if failure.has_code(\"std.net.timed_out\") { return 0 }\n\
                 return 2\n\
             }\n\
             return 3\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-tcp-timeout", 0);
}

#[test]
fn public_async_tcp_idle_read_observes_its_deadline() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/net\n\
         use std/time.Duration\n\
         use std/vec.Vec\n\
         \n\
         async func main(): i32! {\n\
             let address = net.SocketAddress.new(\n\
                 net.IpAddress.from_ipv4(net.Ipv4Address.loopback()),\n\
                 0,\n\
             )\n\
             var listener = await net.bind_tcp(address)?\n\
             let listening = listener.local_address()?\n\
             var client = await net.connect_tcp(listening)?\n\
             let accepted = await listener.accept()?\n\
             var server = move accepted.0\n\
             var waiting: Vec<u8> = Vec [u8.truncate(0)]\n\
             let _count = await server.read_with_timeout(\n\
                 &+waiting,\n\
                 Duration.from_milliseconds(2),\n\
             ) catch failure {\n\
                 if failure.has_code(\"std.net.timed_out\") { return 0 }\n\
                 return 1\n\
             }\n\
             client.close()\n\
             return 2\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-tcp-idle", 0);
}

#[test]
fn public_async_host_connection_uses_one_awaited_result() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/net\n\
         use std/time.Duration\n\
         use std/vec.Vec\n\
         \n\
         async func rejects_invalid_host(): bool {\n\
             let _stream = await net.connect_host(\"\", 80) catch failure {\n\
                 return failure.has_code(\"std.net.invalid_host\")\n\
             }\n\
             return false\n\
         }\n\
         \n\
         async func main(): i32! {\n\
             if !await rejects_invalid_host() { return 1 }\n\
             let address = net.SocketAddress.new(\n\
                 net.IpAddress.from_ipv4(net.Ipv4Address.loopback()),\n\
                 0,\n\
             )\n\
             var listener = await net.bind_tcp(address)?\n\
             let listening = listener.local_address()?\n\
             let pending = net.connect_host_with_timeout(\n\
                 \"localhost\",\n\
                 listening.port(),\n\
                 Duration.from_seconds(1),\n\
             )\n\
             var client = await pending?\n\
             let accepted = await listener.accept_with_timeout(\n\
                 Duration.from_seconds(1),\n\
             )?\n\
             var server = move accepted.0\n\
             await client.write(\"host\".bytes())?\n\
             var buffer: Vec<u8> = Vec [\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
             ]\n\
             let count = await server.read(&+buffer)?\n\
             if count != 4 || buffer[0] != 104 || buffer[1] != 111\n\
                 || buffer[2] != 115 || buffer[3] != 116 { return 2 }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "async-host", 0);
}
