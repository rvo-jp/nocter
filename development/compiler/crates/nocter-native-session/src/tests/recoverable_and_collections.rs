use super::*;

fn recoverable_allocation_test_source() -> &'static str {
    concat!(
        "see ./index.nct\n",
        "use /mem\n",
        "use /string.String\n",
        "test recoverable_integer_text_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let value: i64 = -9223372036854775808\n",
        "    let _text = value.try_to_string(&+allocator) catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.num.allocator\", \"wrong allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.num.allocator\", \"invalid allocator succeeded\")\n",
        "}\n",
        "test recoverable_float_text_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let value: f64 = 1.7976931348623157e308\n",
        "    let _text = value.try_to_string(&+allocator) catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.num.allocator\", \"wrong float allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.num.allocator\", \"invalid allocator formatted a float\")\n",
        "}\n",
        "test recoverable_float_parse_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let _value = f64.try_parse(&+allocator, \"0.1\") catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.num.allocator\", \"wrong float parse allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.num.allocator\", \"invalid allocator parsed a float\")\n",
        "}\n",
        "test recoverable_character_append_is_transactional {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    var text = String.try_with_capacity(&+allocator, 0)?\n",
        "    text.try_push('\\u{1F600}') catch failure {\n",
        "        if !failure.has_code(\"std.mem.invalid_argument\") {\n",
        "            return error.new(\"std.string.allocator\", \"wrong allocator failure\")\n",
        "        }\n",
        "        if (&text as &str) != \"\" {\n",
        "            return error.new(\"std.string.atomicity\", \"failed scalar append changed text\")\n",
        "        }\n",
        "        return\n",
        "    }\n",
        "    return error.new(\"std.string.allocator\", \"invalid allocator succeeded\")\n",
        "}\n",
        "test recoverable_lowercase_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let _text = \"İ\".try_to_lowercase(&+allocator) catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.str.allocator\", \"wrong lowercase allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.str.allocator\", \"invalid allocator lowercased text\")\n",
        "}\n",
        "test recoverable_uppercase_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let _text = \"ß\".try_to_uppercase(&+allocator) catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.str.allocator\", \"wrong uppercase allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.str.allocator\", \"invalid allocator uppercased text\")\n",
        "}\n",
    )
}

const RECOVERABLE_JSON_TEST_SOURCE: &str = concat!(
    "use /json\n",
    "use /mem\n",
    "test recoverable_json_float_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _number = json.Number.try_from_f64(&+allocator, 0.1) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.json.allocator\", \"wrong float allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.json.allocator\", \"invalid allocator created a number\")\n",
    "}\n",
    "test recoverable_json_generation_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let value = json.Value.null\n",
    "    let _text = json.try_stringify(&+allocator, &value) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.json.allocator\", \"wrong generation allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.json.allocator\", \"invalid allocator generated JSON\")\n",
    "}\n",
);

const RECOVERABLE_URL_TEST_SOURCE: &str = concat!(
    "use /mem\n",
    "use /url.Url\n",
    "test recoverable_url_parse_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _value = Url.try_parse(&+allocator, \"https://example.com/a\") catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.url.allocator\", \"wrong parse allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.url.allocator\", \"invalid allocator parsed a URL\")\n",
    "}\n",
    "test recoverable_url_resolution_propagates_allocator_failure {\n",
    "    let base = Url.parse(\"https://example.com/a/b\")?\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _value = base.try_resolve(&+allocator, \"../c\") catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.url.allocator\", \"wrong resolve allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.url.allocator\", \"invalid allocator resolved a URL\")\n",
    "}\n",
    "test recoverable_url_text_generation_propagates_allocator_failure {\n",
    "    let value = Url.parse(\"https://example.com/a\")?\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _text = value.try_to_string(&+allocator) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.url.allocator\", \"wrong format allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.url.allocator\", \"invalid allocator formatted a URL\")\n",
    "}\n",
    "test recoverable_request_target_propagates_allocator_failure {\n",
    "    let value = Url.parse(\"https://example.com/a?q\")?\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _text = value.try_request_target(&+allocator) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.url.allocator\", \"wrong target allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.url.allocator\", \"invalid allocator made a request target\")\n",
    "}\n",
    "test recoverable_url_authority_propagates_allocator_failure {\n",
    "    let value = Url.parse(\"https://example.com:8443/a\")?\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _text = value.try_authority(&+allocator) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.url.allocator\", \"wrong authority allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.url.allocator\", \"invalid allocator made an authority\")\n",
    "}\n",
);

const RECOVERABLE_NET_TEST_SOURCE: &str = concat!(
    "use /mem\n",
    "use /net\n",
    "test recoverable_numeric_resolution_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _addresses = net.try_resolve(&+allocator, \"127.0.0.1\", 80) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.net.allocator\", \"wrong numeric allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.net.allocator\", \"invalid allocator resolved numeric host\")\n",
    "}\n",
    "test recoverable_named_resolution_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _addresses = net.try_resolve(&+allocator, \"localhost\", 80) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.net.allocator\", \"wrong named-host allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.net.allocator\", \"invalid allocator resolved named host\")\n",
    "}\n",
);

#[test]
fn standard_recoverable_allocation_contracts_preserve_failure_atomicity() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");

    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"numeric\", module: \"./num\" }\n",
        "#test: { name: \"json-failure\", module: \".\" }\n",
        "see ./allocator_failure_json_tests.nct\n",
        "see ./allocator_failure_url_tests.nct\n",
        "see ./allocator_failure_net_tests.nct\n",
    ));

    let mut mem_contract = fs::read_to_string(standard_root.join("mem/index.nct")).unwrap();
    mem_contract.push_str("\npub(/) func failing_try_allocator_for_test(): TryAllocator\n");

    let mut mem_storage = fs::read_to_string(standard_root.join("mem/storage.nct")).unwrap();
    mem_storage.push_str(concat!(
        "\nfunc failing_try_allocator_for_test(): TryAllocator {\n",
        "    return TryAllocator { state: 0, kind: 99 }\n",
        "}\n",
    ));

    let num_contract = format!(
        "see ./allocator_failure_tests.nct\n{}",
        fs::read_to_string(standard_root.join("num/index.nct")).unwrap()
    );
    let num_failure_tests = recoverable_allocation_test_source();

    let mut overlay = SourceOverlay::builder();
    for (path, source) in [
        (standard_root.join("index.nct"), root_source),
        (standard_root.join("mem/index.nct"), mem_contract),
        (standard_root.join("mem/storage.nct"), mem_storage),
        (standard_root.join("num/index.nct"), num_contract),
        (
            standard_root.join("num/allocator_failure_tests.nct"),
            num_failure_tests.to_string(),
        ),
        (
            standard_root.join("allocator_failure_json_tests.nct"),
            RECOVERABLE_JSON_TEST_SOURCE.to_string(),
        ),
        (
            standard_root.join("allocator_failure_url_tests.nct"),
            RECOVERABLE_URL_TEST_SOURCE.to_string(),
        ),
        (
            standard_root.join("allocator_failure_net_tests.nct"),
            RECOVERABLE_NET_TEST_SOURCE.to_string(),
        ),
    ] {
        overlay
            .insert_source(path, SourceOverride::new(source.into_bytes()))
            .unwrap();
    }

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
    assert_eq!(compiled.targets().len(), 2);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("allocator failure tests failed native compilation")
        };
        case_count += cases.len();
        for case in cases {
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 32);
}

#[test]
fn standard_json_phase_three_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"unicode\", module: \"./internal/utf8\" }\n",
        "#test: { name: \"json\", module: \"./json\" }\n",
    ));
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
            ModuleIdentity::new(standard_package.clone(), ["internal", "utf8"]),
            ModuleIdentity::new(standard_package.clone(), ["json"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 2);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("standard JSON Phase 3 tests failed native compilation")
        };
        case_count += cases.len();
        for case in cases {
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 25);
}

#[test]
fn standard_json_writer_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source("index.nct", JSON_WRITER_CONTRACT_TEST_SOURCE);
    package_root.source("implementation.nct", JSON_WRITER_IMPLEMENTATION_TEST_SOURCE);
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:json-writer-tests");
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
        panic!("standard JSON BlockingWriter tests failed native compilation")
    };
    assert_eq!(cases.len(), 3);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_map_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "index.nct",
        concat!(
            "//! Public Map contract tests.\n",
            "#package: { name: \"map-tests\", version: \"0.0.0\", }\n",
            "#test: { name: \"map\", module: \"./tests\" }\n",
        ),
    );
    let contract_source = "use std/hash.{Hash, HashState}\n\
         use std/mem\n\
         use std/string.String\n\
         see ./implementation.nct\n\
         pub struct CollisionKey { pub id: i32 }\n\
         pub struct Marker {}\n\
         instance CollisionKey {\n\
             impl Hash\n\
             pub operator (&self == other: &Self): bool\n\
             pub noalloc method &self.hash_into(state: &+HashState): void\n\
         }\n\
         instance Marker {\n\
             impl Hash\n\
             pub operator (&self == other: &Self): bool\n\
             pub noalloc method &self.hash_into(state: &+HashState): void\n\
         }\n";
    package_root.source("tests/index.nct", contract_source);
    package_root.source("tests/implementation.nct", MAP_PHASE3_TEST_SOURCE);
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:map-tests");
    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
        vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package, ["tests"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard map tests failed native compilation")
    };
    assert_eq!(cases.len(), 9);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn constants_cross_fixed_array_checking_and_native_lowering() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "const WIDTH: usize = 1 + 1\n",
            "const ANSWER: i32 = 40 + 2\n",
            "const LABEL: &str = \"Nocter\"\n",
            "func main(): i32 {\n",
            "    let values: [i32; WIDTH] = [ANSWER, ANSWER]\n",
            "    if LABEL == \"Nocter\" { return values[0] }\n",
            "    return 1\n",
            "}\n",
        ),
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
    assert!(!image.image().bytes().is_empty());
}

#[test]
fn immutable_static_arrays_cross_readonly_data_and_native_relocation() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "static VALUES: [u32; 2] = [65, 90]\n",
            "static LABELS: [&str; 2] = [\"first\", \"second\"]\n",
            "func main(): i32 {\n",
            "    if VALUES[1] != 90 { return 1 }\n",
            "    let first = LABELS[0]\n",
            "    let second = LABELS[1]\n",
            "    if first != \"first\" { return 2 }\n",
            "    if second != \"second\" { return 3 }\n",
            "    return 0\n",
            "}\n",
        ),
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
    execute_native_test(image.image(), &package_root.0, "immutable-static-data");
}
