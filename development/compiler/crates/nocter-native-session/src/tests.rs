#![allow(clippy::disallowed_methods)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_filesystem::{SourceOverlay, SourceOverride};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::PrimitiveRole;
use nocter_test_support::PUBLIC_PACKAGE_EXAMPLES;

use super::{
    NativeImage, NativeImageSetCompileRequest, NativeTestCompileRequest, NativeTestTargetOutcome,
    compile_native_image, compile_native_images, compile_native_tests,
};
use nocter_session::{
    ExecutableCompileRequest, analyze_incomplete_syntax, analyze_target,
    bundled_standard_toolchain, compile_target,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

const DIRECTORY_RECORD_TEST_SOURCE: &[u8] = br#"see ./directory.nct

use /internal/os/darwin.{dirent_inode_offset, dirent_name_length_offset}
use /internal/os/darwin.{dirent_name_offset, dirent_record_length_offset}
use /internal/os/darwin.{dirent_type_offset, dirent_type_regular}
use /internal/ptr.{from_addr, store_u8_to_ptr}
use /mem.page_try_allocator
use /path.Utf8Path
use /ptr.addr

func test_reader(
    record_len: u8,
    name_len: u8,
    name_byte: u8,
    terminator: u8,
): ReadDir! {
    var allocator = page_try_allocator()
    var buffer = allocator.try_alloc(64, 8)?
    let address = addr(buffer.bytes_mut().ptr())
    let type_pointer: *u8 = from_addr(address)
    store_u8_to_ptr(type_pointer, dirent_inode_offset, 1)
    store_u8_to_ptr(type_pointer, dirent_record_length_offset, record_len)
    store_u8_to_ptr(type_pointer, dirent_record_length_offset + 1, 0)
    store_u8_to_ptr(type_pointer, dirent_name_length_offset, name_len)
    store_u8_to_ptr(type_pointer, dirent_name_length_offset + 1, 0)
    store_u8_to_ptr(type_pointer, dirent_type_offset, dirent_type_regular)
    store_u8_to_ptr(type_pointer, dirent_name_offset, name_byte)
    store_u8_to_ptr(type_pointer, dirent_name_offset + name_len as usize, terminator)
    let base = Utf8Path.new(".")?
    return ReadDir {
        fd: 999999,
        is_open: true,
        is_finished: false,
        base: move base,
        buffer: move buffer,
        buffer_offset: 0,
        buffer_len: 24,
    }
}

test malformed_record_is_terminal {
    var reader = test_reader(0, 0, 0, 0)?
    let _entry = reader.next() catch failure {
        if !failure.has_code("std.fs.invalid_directory_record") {
            return error.new("test.wrong_error", "malformed record reported the wrong error")
        }
        let _after_failure = reader.next()? otherwise { return }
        return error.new("test.not_terminal", "malformed record did not end the stream")
    } otherwise {
        return error.new("test.unexpected_eof", "malformed record produced end of stream")
    }
    return error.new("test.unexpected_entry", "malformed record produced an entry")
}

test invalid_utf8_name_is_terminal {
    var reader = test_reader(24, 1, 255, 0)?
    let _entry = reader.next() catch failure {
        if !failure.has_code("std.fs.invalid_utf8_name") {
            return error.new("test.wrong_error", "invalid UTF-8 reported the wrong error")
        }
        let _after_failure = reader.next()? otherwise { return }
        return error.new("test.not_terminal", "invalid UTF-8 did not end the stream")
    } otherwise {
        return error.new("test.unexpected_eof", "invalid UTF-8 produced end of stream")
    }
    return error.new("test.unexpected_entry", "invalid UTF-8 produced an entry")
}
"#;

const COLLECTION_ORDERING_TEST_SOURCE: &str = r#"use std/string.String
use std/vec.Vec

struct Counter {
    value: i32
}

struct Counters {
    a: Counter
    b: Counter
    c: Counter
    d: Counter
}

struct Unit {}

struct Tracked {
    key: i32
    serial: i32
    counter: &+Counter
}

instance Tracked {
    operator (&self < other: &Self): bool {
        return self.key < other.key
    }
}

drop Tracked(&+self) {
    self.counter.value += 1
    return
}

func check_slice_view(): i32 {
    var values = Vec [4, 1, 3, 2, 2]
    let view: &+[i32] = &+values as &+[i32]
    view.sort()
    if values[0] != 1 || values[1] != 2 || values[2] != 2 { return 1 }
    if values[3] != 3 || values[4] != 4 { return 2 }

    var empty: Vec<i32> = Vec.empty()
    empty.sort()
    var single = Vec [7]
    single.sort()
    if single[0] != 7 { return 3 }
    var ordered = Vec [1, 2, 3]
    ordered.sort()
    if ordered[0] != 1 || ordered[1] != 2 || ordered[2] != 3 { return 4 }
    return 0
}

func check_vec_and_coercion(): i32 {
    var values: Vec<i32> = Vec.empty()
    var value: i32 = 128
    while value != 0 {
        values.push(value)
        value -= 1
    }
    values.sort()
    var index: usize = 0
    var expected: i32 = 1
    while index < 128 {
        if values[index] != expected { return 1 }
        index += 1
        expected += 1
    }
    return 0
}

func check_string_coercion(): i32 {
    var values = Vec [String.copy("gamma"), String.copy("alpha"), String.copy("beta")]
    values.sort()
    if (&values[0] as &str) != "alpha" { return 1 }
    if (&values[1] as &str) != "beta" { return 2 }
    if (&values[2] as &str) != "gamma" { return 3 }
    return 0
}

func sort_tracked(counters: &+Counters): i32 {
    var values = Vec [
        Tracked { key: 3, serial: 0, counter: &+counters.a },
        Tracked { key: 1, serial: 1, counter: &+counters.b },
        Tracked { key: 2, serial: 2, counter: &+counters.c },
        Tracked { key: 2, serial: 3, counter: &+counters.d },
    ]
    values.sort()
    if values[0].key != 1 || values[1].key != 2 { return 1 }
    if values[2].key != 2 || values[3].key != 3 { return 2 }
    let duplicates_preserved =
        (values[1].serial == 2 && values[2].serial == 3) ||
        (values[1].serial == 3 && values[2].serial == 2)
    if !duplicates_preserved { return 3 }
    return 0
}

func check_move_only_destruction(): i32 {
    var counters = Counters {
        a: Counter { value: 0 },
        b: Counter { value: 0 },
        c: Counter { value: 0 },
        d: Counter { value: 0 },
    }
    let sort_result = sort_tracked(&+counters)
    if sort_result != 0 { return sort_result }
    if counters.a.value != 1 || counters.b.value != 1 { return 4 }
    if counters.c.value != 1 || counters.d.value != 1 { return 5 }
    return 0
}

func check_zero_sized_vec(): i32 {
    var values: Vec<Unit> = Vec.with_capacity(2)
    values.push(Unit {})
    values.push(Unit {})
    values.insert(1, Unit {})
    if values.len() != 3 { return 1 }
    let removed = values.remove(1) otherwise { return 2 }
    let popped = values.pop() otherwise { return 3 }
    if values.len() != 1 { return 4 }
    values.clear()
    if values.len() != 0 { return 5 }
    return 0
}

func main(): i32 {
    let slice = check_slice_view()
    if slice != 0 { return 10 + slice }
    let vector = check_vec_and_coercion()
    if vector != 0 { return 20 + vector }
    let strings = check_string_coercion()
    if strings != 0 { return 30 + strings }
    let tracked = check_move_only_destruction()
    if tracked != 0 { return 40 + tracked }
    let zero_sized = check_zero_sized_vec()
    if zero_sized != 0 { return 50 + zero_sized }
    return 0
}
"#;

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nocter-session-package-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn source(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn bundled_standard_library_crosses_the_complete_target_session() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
    let package = PackageIdentity::new("toolchain:std");
    let resolved = resolved_standard(&root, &package);
    let roots = module_roots(&root)
        .into_iter()
        .map(|path| ModuleIdentity::new(package.clone(), path))
        .collect();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![resolved]),
        roots,
        bundled_standard_toolchain(&package),
    ))
    .unwrap();
    let diagnostics = unit.syntax_diagnostics();
    let source_names = unit
        .sources()
        .iter()
        .map(|source| (source.id(), source.name().as_str()))
        .collect::<Vec<_>>();
    assert!(
        diagnostics.is_empty(),
        "bundled standard library has syntax diagnostics: {diagnostics:#?}\nsources: {source_names:#?}"
    );
    let compiled = compile_target(&unit).unwrap();

    assert_eq!(
        compiled.program().toolchain().primitives().bindings().len(),
        PrimitiveRole::ALL.len()
    );
    assert_eq!(
        compiled.program().checked().bodies().len(),
        compiled
            .program()
            .checked()
            .graph()
            .declarations()
            .bodies()
            .len()
    );
}

#[test]
fn standard_string_concat_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "func main(): i32 {\n",
            "    let text = String.concat(\"No\", \"cter\")\n",
            "    if (&text as &str) == \"Nocter\" { return 42 }\n",
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

    let image = compile_native_image(ExecutableCompileRequest::only(&unit)).unwrap();
    assert!(!image.image().bytes().is_empty());
}

#[test]
fn standard_directory_stream_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/fs.{FileType, read_dir}

func open_and_drop(): void! {
    let stream = read_dir(".")?
    return
}

func open_fails_with(path: &str, code: &str): bool {
    let _stream = read_dir(path) catch failure {
        return failure.has_code(code)
    }
    return false
}

func inspect_directory(): i32! {
    var stream = read_dir(".")?
    var saw_file = false
    var saw_directory = false
    var saw_symlink = false
    var batch_count: usize = 0
    while true {
        let entry = stream.next()? otherwise { break }
        let name = entry.file_name()
        let path: &str = entry.path()
        if name == "." || name == ".." { return 2 }
        if name == "regular.txt" {
            if entry.file_type() is FileType.regular { saw_file = true }
            if !saw_file { return 3 }
            if path != "./regular.txt" { return 4 }
        }
        if name == "nested" {
            if entry.file_type() is FileType.directory { saw_directory = true }
            if !saw_directory { return 5 }
        }
        if name == "link" {
            if entry.file_type() is FileType.symlink { saw_symlink = true }
            if !saw_symlink { return 6 }
        }
        if name.starts_with("batch-") { batch_count += 1 }
    }
    if !saw_file || !saw_directory || !saw_symlink || batch_count != 700 { return 7 }
    if !open_fails_with("missing", "std.io.not_found") { return 8 }
    if !open_fails_with("regular.txt", "std.io.not_directory") { return 9 }

    var attempts: usize = 0
    while attempts < 512 {
        open_and_drop()?
        attempts += 1
    }

    var closed = read_dir(".")?
    closed.close()
    let _after_close = closed.next()? otherwise { return 42 }
    return 11
}

func main(): i32 {
    return inspect_directory() catch _ { return 12 }
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "directory stream fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let image = compile_native_image(ExecutableCompileRequest::only(&unit)).unwrap();
    execute_directory_stream(image.image(), &package_root.0, 42);
}

#[test]
fn standard_streaming_lines_cross_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/io.{File, Writer}
use std/io/buffer.{BufReader, BufWriter}
use std/string.String

func check_lines(): i32! {
    var reader = BufReader.with_capacity(File.open("lines.txt")?, 3)
    var line = String.with_capacity(64)
    let original_capacity = line.capacity()

    if !reader.read_line_into(&+line)? || (&line as &str) != "" { return 1 }
    if !reader.read_line_into(&+line)? || (&line as &str) != "alpha" { return 2 }
    if !reader.read_line_into(&+line)? || (&line as &str) != "lone\rbeta" { return 3 }
    if !reader.read_line_into(&+line)? || (&line as &str) != "😀 split" { return 4 }
    let final_line = reader.read_line()? otherwise { return 5 }
    if (&final_line as &str) != "final" { return 6 }
    if reader.read_line_into(&+line)? { return 7 }
    if (&line as &str) != "" { return 8 }
    if line.capacity() != original_capacity { return 9 }
    let _after_eof = reader.read_line()? otherwise { return 0 }
    return 10
}

func check_invalid_utf8(): i32! {
    var reader = BufReader.with_capacity(File.open("invalid.txt")?, 2)
    var line = String.copy("sentinel")
    if !reader.read_line_into(&+line)? || (&line as &str) != "good" { return 1 }
    let _present = reader.read_line_into(&+line) catch failure {
        if !failure.has_code("std.string.invalid_utf8") { return 2 }
        if (&line as &str) != "" { return 3 }
        let _after_failure = reader.read_line()? otherwise { return 0 }
        return 4
    }
    return 5
}

func check_zero_capacity_and_close(): i32! {
    var reader = BufReader.with_capacity(File.open("single.txt")?, 0)
    let line = reader.read_line()? otherwise { return 1 }
    if (&line as &str) != "z" { return 2 }
    let _after_eof = reader.read_line()? otherwise {
        var closed = BufReader.new(File.open("single.txt")?)
        closed.close()
        let _after_close = closed.read_line()? otherwise { return 0 }
        return 3
    }
    return 4
}

func check_closed_output(): i32! {
    var file = File.create("closed-file.txt")?
    file.close()
    file.write_text("not written") catch failure {
        if !failure.has_code("std.io.closed") { return 1 }
        var writer = BufWriter.with_capacity(File.create("writer.txt")?, 0)
        writer.write_text("abc")?
        writer.close()?
        writer.write_text("not written") catch writer_failure {
            if !writer_failure.has_code("std.io.closed") { return 2 }
            return 0
        }
        return 3
    }
    return 4
}

func main(): i32 {
    let lines = check_lines() catch _ { return 20 }
    if lines != 0 { return lines }
    let invalid = check_invalid_utf8() catch _ { return 21 }
    if invalid != 0 { return 10 + invalid }
    let terminal = check_zero_capacity_and_close() catch _ { return 22 }
    if terminal != 0 { return 30 + terminal }
    let output = check_closed_output() catch _ { return 23 }
    if output != 0 { return 40 + output }
    return 42
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "streaming line fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let image = compile_native_image(ExecutableCompileRequest::only(&unit)).unwrap();
    execute_streaming_lines(image.image(), &package_root.0, 42);
}

#[test]
fn standard_collection_ordering_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source("main.nct", COLLECTION_ORDERING_TEST_SOURCE);
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "collection ordering fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let image = compile_native_image(ExecutableCompileRequest::only(&unit)).unwrap();
    execute_native_test(image.image(), &package_root.0, "collection-ordering");
}

#[test]
fn standard_directory_record_failures_are_terminal_in_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"directory-records\", module: \"./fs\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    overlay
        .insert_source(
            standard_root.join("fs/directory_phase0_test.nct"),
            SourceOverride::new(DIRECTORY_RECORD_TEST_SOURCE.to_vec()),
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
            ModuleIdentity::new(standard_package.clone(), ["fs"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_native_tests(NativeTestCompileRequest::all(&unit)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("directory record tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
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
            "const width: usize = 1 + 1\n",
            "const answer: i32 = 40 + 2\n",
            "const label: &str = \"Nocter\"\n",
            "func main(): i32 {\n",
            "    let values: [i32; width] = [answer, answer]\n",
            "    if label == \"Nocter\" { return values[0] }\n",
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

    let image = compile_native_image(ExecutableCompileRequest::only(&unit)).unwrap();
    assert!(!image.image().bytes().is_empty());
}

#[test]
fn body_failure_retains_preparation_and_exact_typed_interruption() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func helper(): i32 { 1 }\nfunc main(input: i32): void {\n    input.missing()\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let failure = analyze_target(&unit).unwrap_err();
    assert!(!failure.error().source_diagnostics().is_empty());
    let body_analysis = failure
        .semantic_evidence()
        .unwrap()
        .body_analysis()
        .expect("expected body evidence");
    let prepared = body_analysis.prepared();
    assert!(!prepared.graph().declarations().callables().is_empty());
    assert!(!body_analysis.body_names().is_empty());
    assert!(!body_analysis.source_index().is_empty());
    let interruption = body_analysis.interruptions().next().unwrap();
    assert_eq!(
        interruption.origin().span(),
        failure
            .error()
            .source_diagnostics()
            .first()
            .unwrap()
            .primary()
            .span()
    );
    assert!(matches!(
        interruption.kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn name_failure_retains_lexical_state_without_claiming_body_preparation() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func main(input: i32): void {\n    let before = input\n    unknown\n    let after = input\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let failure = analyze_target(&unit).unwrap_err();
    assert_eq!(failure.error().source_diagnostics()[0].code(), "E0340");
    let recovery = failure
        .semantic_evidence()
        .unwrap()
        .name_analysis()
        .expect("expected name evidence");
    assert!(!recovery.graph().declarations().callables().is_empty());
    assert!(!recovery.body_names().is_empty());
    assert!(!recovery.source_index().is_empty());
}

#[test]
fn interface_implementation_failure_retains_declarations_without_claiming_later_semantics() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "pub interface Readable { pub method &self.read(): i32 }\n",
            "struct Value {}\n",
            "instance Value { impl Readable }\n",
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

    let failure = analyze_target(&unit).unwrap_err();
    assert_eq!(failure.error().source_diagnostics()[0].code(), "E0350");
    let declarations = failure
        .semantic_evidence()
        .unwrap()
        .declaration_analysis()
        .expect("expected declaration evidence");
    assert!(
        !declarations
            .graph()
            .declarations()
            .interface_implementations()
            .is_empty()
    );
    assert!(!declarations.source_index().is_empty());
}

#[test]
fn incomplete_member_syntax_retains_typed_receiver_context() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "struct Text { value: i32 }\ninstance Text { pub method &self.len(): usize { 0 } }\nfunc inspect(value: &Text): void {\n    value.\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_incomplete_syntax(&unit).expect("incomplete syntax analysis");
    let semantic = analysis.semantic_evidence().expect("typed syntax recovery");
    let recovery = semantic.body_analysis().expect("expected body evidence");
    assert!(matches!(
        recovery.interruptions().next().unwrap().kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn incomplete_declaration_syntax_cannot_enter_body_recovery() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source("main.nct", "func broken(: void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_incomplete_syntax(&unit).expect("incomplete syntax analysis");
    assert!(analysis.semantic_evidence().is_none());
}

#[test]
fn incomplete_syntax_preserves_an_independent_declaration_failure() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "pub interface Readable { pub method &self.read(): i32 }\n",
            "struct Value {}\n",
            "instance Value { impl Readable }\n",
            "func inspect(value: &Value): void {\n",
            "    value.\n",
            "    return\n",
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

    assert!(unit.has_syntax_errors());
    let analysis = analyze_incomplete_syntax(&unit).expect("incomplete syntax analysis");
    assert_eq!(
        analysis
            .failure()
            .unwrap()
            .source_diagnostics()
            .first()
            .unwrap()
            .code(),
        "E0350"
    );
    let semantic = analysis.semantic_evidence().expect("declaration analysis");
    let declarations = semantic
        .declaration_analysis()
        .expect("expected declaration evidence");
    assert!(
        !declarations
            .graph()
            .declarations()
            .interface_implementations()
            .is_empty()
    );
}

#[test]
fn incomplete_syntax_preserves_an_earlier_name_failure() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "struct Text {}\n",
            "func inspect(value: &Text): void {\n",
            "    unknown\n",
            "    value.\n",
            "    return\n",
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

    assert!(unit.has_syntax_errors());
    let analysis = analyze_incomplete_syntax(&unit).expect("incomplete syntax analysis");
    assert_eq!(
        analysis
            .failure()
            .unwrap()
            .source_diagnostics()
            .first()
            .unwrap()
            .code(),
        "E0340"
    );
    let semantic = analysis.semantic_evidence().expect("name analysis");
    let names = semantic.name_analysis().expect("expected name evidence");
    assert!(!names.body_names().is_empty());
}

#[test]
fn every_public_single_file_example_crosses_the_complete_target_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let examples = compiler_root.join("../../examples");
    let package = PackageIdentity::new("toolchain:std");
    let mut sources = fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nct"))
        .collect::<Vec<_>>();
    sources.sort();
    assert!(!sources.is_empty());

    for source in sources {
        let unit = discover(DiscoveryRequest::single_file(
            CompilationTarget::Arm64Darwin,
            &source,
            package_graph(vec![resolved_standard(&standard_root, &package)]),
            bundled_standard_toolchain(&package),
        ))
        .unwrap_or_else(|error| panic!("{} failed discovery: {error:?}", source.display()));
        compile_native_image(ExecutableCompileRequest::only(&unit))
            .unwrap_or_else(|error| panic!("{} failed compilation: {error:?}", source.display()));
    }
}

#[test]
fn every_public_package_example_crosses_the_complete_target_session() {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let examples_root = compiler.join("../../examples");
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut discovered = fs::read_dir(&examples_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("index.nct").is_file())
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut contracted = PUBLIC_PACKAGE_EXAMPLES
        .iter()
        .map(|contract| contract.directory().to_owned())
        .collect::<Vec<_>>();
    discovered.sort();
    contracted.sort();
    assert_eq!(
        discovered, contracted,
        "public package contract is incomplete"
    );

    for contract in PUBLIC_PACKAGE_EXAMPLES {
        let package_root = examples_root.join(contract.directory());
        let example_package = PackageIdentity::new(contract.package_identity());
        let example = ResolvedPackageSpec::new(example_package.clone(), &package_root)
            .with_standard_dependency(standard_package.clone());
        let unit = discover(DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            package_graph(vec![
                example,
                resolved_standard(&standard_root, &standard_package),
            ]),
            vec![ModuleIdentity::new(
                example_package.clone(),
                Vec::<&str>::new(),
            )],
            bundled_standard_toolchain(&standard_package),
        ))
        .unwrap_or_else(|error| panic!("{} failed discovery: {error:?}", contract.directory()));
        let target = compile_native_image(ExecutableCompileRequest::named(
            &unit,
            contract.executable(),
        ))
        .unwrap_or_else(|error| panic!("{} failed compilation: {error:?}", contract.directory()));

        assert_eq!(target.identity().name(), contract.executable());
        assert_eq!(target.identity().package(), &example_package);
        assert!(!target.image().bytes().is_empty());
    }
}

#[test]
fn all_root_executables_share_one_target_compilation_and_keep_declaration_order() {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "index.nct",
        concat!(
            "//! Multi executable package.\n",
            "#package: { name: \"multi\", version: \"0.0.0\", }\n",
            "#executable: { name: \"first\", module: \"./first\" }\n",
            "#executable: { name: \"second\", module: \"./second\" }\n",
        ),
    );
    package_root.source("first/index.nct", "func main(): void { return }\n");
    package_root.source("second/index.nct", "func main(): void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:multi");
    let compile = |reverse_input: bool| {
        let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
            .with_standard_dependency(standard_package.clone());
        let mut packages = vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ];
        let mut roots = vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["first"]),
            ModuleIdentity::new(package.clone(), ["second"]),
        ];
        if reverse_input {
            packages.reverse();
            roots.reverse();
        }
        let unit = discover(DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            package_graph(packages),
            roots,
            bundled_standard_toolchain(&standard_package),
        ))
        .unwrap();
        compile_native_images(NativeImageSetCompileRequest::all(&unit)).unwrap()
    };

    let image_set = compile(false);
    let reversed_image_set = compile(true);
    assert_eq!(
        image_set
            .entries()
            .iter()
            .map(|entry| entry.identity().name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.identity().package() == &package)
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe]))
    );
    assert_eq!(
        image_set
            .entries()
            .iter()
            .map(|entry| (entry.identity(), entry.image().bytes()))
            .collect::<Vec<_>>(),
        reversed_image_set
            .entries()
            .iter()
            .map(|entry| (entry.identity(), entry.image().bytes()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn native_test_set_preserves_target_and_case_declaration_identity() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "index.nct",
        concat!(
            "//! Test package.\n",
            "#package: { name: \"tests\", version: \"0.0.0\", }\n",
            "#test: { name: \"unit\", module: \"./unit\" }\n",
            "#test: { name: \"integration\", module: \"./integration\" }\n",
        ),
    );
    package_root.source(
        "unit/index.nct",
        "test first { return }\ntest second { return }\n",
    );
    package_root.source("integration/index.nct", "test external { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:tests");
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
            ModuleIdentity::new(package.clone(), ["unit"]),
            ModuleIdentity::new(package.clone(), ["integration"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_native_tests(NativeTestCompileRequest::all(&unit)).unwrap();
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| target.identity().name())
            .collect::<Vec<_>>(),
        ["unit", "integration"]
    );
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .map(|case| case.identity().name())
                    .collect::<Vec<_>>(),
                NativeTestTargetOutcome::CompileFailed(error) => {
                    panic!("test target failed native compilation: {error}")
                }
            })
            .collect::<Vec<_>>(),
        [vec!["first", "second"], vec!["external"]]
    );
    assert!(compiled.targets().iter().all(|target| {
        target.identity().package() == &package
            && match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .all(|case| case.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe])),
                NativeTestTargetOutcome::CompileFailed(_) => false,
            }
    }));

    let selected =
        compile_native_tests(NativeTestCompileRequest::case(&unit, "unit", "second")).unwrap();
    let NativeTestTargetOutcome::Compiled(cases) = selected.targets()[0].outcome() else {
        panic!("selected case failed native compilation")
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].identity().name(), "second");
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(package.clone(), root).with_standard_dependency(package.clone())
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load(packages).unwrap()
}

fn package_graph_with_overlay(
    packages: Vec<ResolvedPackageSpec>,
    overlay: SourceOverlay,
) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load_with_source_overlay(packages, overlay).unwrap()
}

fn module_roots(root: &Path) -> Vec<Vec<Box<str>>> {
    let mut pending = vec![(root.to_path_buf(), Vec::new())];
    let mut modules = Vec::new();
    while let Some((directory, path)) = pending.pop() {
        if directory.join("index.nct").is_file() {
            modules.push(path.clone());
        }
        let mut children = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<PathBuf>>();
        children.sort();
        for child in children.into_iter().rev() {
            let mut child_path = path.clone();
            child_path.push(
                child
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
                    .into(),
            );
            pending.push((child, child_path));
        }
    }
    modules.sort();
    modules
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_directory_stream(image: &NativeImage, root: &Path, expected: i32) {
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::Command;

    let executable = root.join("directory-stream");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("regular.txt"), b"text").unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    symlink("regular.txt", root.join("link")).unwrap();
    for index in 0..700 {
        fs::write(root.join(format!("batch-{index:04}")), b"").unwrap();
    }

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "directory stream exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_streaming_lines(image: &NativeImage, root: &Path, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("streaming-lines");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        root.join("lines.txt"),
        b"\nalpha\r\nlone\rbeta\n\xf0\x9f\x98\x80 split\nfinal",
    )
    .unwrap();
    fs::write(root.join("invalid.txt"), b"good\nbad\xff\nlater\n").unwrap();
    fs::write(root.join("single.txt"), b"z").unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "streaming line reader exited with {status:?}"
    );
    assert_eq!(fs::read(root.join("closed-file.txt")).unwrap(), b"");
    assert_eq!(fs::read(root.join("writer.txt")).unwrap(), b"abc");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_native_test(image: &NativeImage, root: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join(name);
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "native test {name} exited with {status:?}"
    );
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_directory_stream(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_streaming_lines(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_native_test(_image: &NativeImage, _root: &Path, _name: &str) {}
