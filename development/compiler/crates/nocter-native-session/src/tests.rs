use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveredUnit, DiscoveryRequest};
use nocter_filesystem::{SourceOverlay, SourceOverride};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::PrimitiveRole;
use nocter_standard_profile::bundled_standard_toolchain;
use nocter_test_support::PUBLIC_PACKAGE_EXAMPLES;

use super::{
    NativeImage, NativeImageSetCompileRequest, NativeTestCompileRequest, NativeTestTargetOutcome,
    compile_native_image, compile_native_images, compile_native_tests,
};
use nocter_session::{AnalyzedUnit, AnalyzedUnitStatus, CompiledTarget, ExecutableCompileRequest};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

const JSON_WRITER_CONTRACT_TEST_SOURCE: &str = r#"//! Public JSON Writer contract tests.
#package: { name: "json-writer-tests", version: "0.0.0", }
#test: { name: "writer", module: "." }
use std/io.Writer
use std/string.String
see ./implementation.nct
pub struct RecordingWriter
construct RecordingWriter {
    pub func accepting(): Self
    pub func failing_after(write_count: usize): Self
}
instance RecordingWriter {
    impl Writer
    pub method &self.text(): &str
}
"#;

const JSON_WRITER_IMPLEMENTATION_TEST_SOURCE: &str = r#"see ./index.nct
use std/json
use std/mem
use std/string.String
struct RecordingWriter {
    output: String
    write_count: usize
    failure_at: usize
}
construct RecordingWriter {
    func accepting(): Self {
        return RecordingWriter { output: String.empty(), write_count: 0, failure_at: 1000000 }
    }
    func failing_after(write_count: usize): Self {
        return RecordingWriter { output: String.empty(), write_count: 0, failure_at: write_count }
    }
}
instance RecordingWriter {
    method &+self.write(bytes: &[u8]): void! {
        if self.write_count >= self.failure_at {
            return error.new("test.destination", "destination rejected JSON bytes")
        }
        self.output.try_push_utf8(bytes)?
        self.write_count += 1
        return
    }
    method &self.text(): &str { return &self.output as &str }
}
test write_streams_the_shared_compact_spelling {
    let value = json.parse("{\"items\":[1,\"é\"]}")?
    var writer = RecordingWriter.accepting()
    json.write(&+writer, &value)?
    if writer.text() != "{\"items\":[1,\"é\"]}" {
        return error.new("test.output", "Writer spelling diverged from String generation")
    }
    return
}
test try_write_uses_the_selected_traversal_allocator {
    let value = json.parse("[null,true,-0]")?
    var allocator = mem.page_try_allocator()
    var writer = RecordingWriter.accepting()
    json.try_write(&+allocator, &+writer, &value)?
    if writer.text() != "[null,true,-0]" {
        return error.new("test.output", "recoverable Writer spelling changed")
    }
    return
}
test write_returns_destination_failure_after_partial_output {
    let value = json.parse("[1,2]")?
    var writer = RecordingWriter.failing_after(2)
    json.write(&+writer, &value) catch failure {
        if !failure.has_code("test.destination") || writer.text() == "[1,2]" {
            return error.new("test.failure", "destination failure identity or partial output changed")
        }
        return
    }
    return error.new("test.failure", "destination failure was not returned")
}
"#;

const IO_WRITER_CONTRACT_TEST_SOURCE: &str = r#"//! Public Writer line-adapter tests.
#package: { name: "io-writer-tests", version: "0.0.0", }
#test: { name: "writer", module: "." }
use std/io.Writer
use std/string.String
see ./implementation.nct
pub struct RecordingWriter
construct RecordingWriter {
    pub func accepting(): Self
    pub func failing_after(write_count: usize): Self
}
instance RecordingWriter {
    impl Writer
    pub method &self.text(): &str
}
"#;

const IO_WRITER_IMPLEMENTATION_TEST_SOURCE: &str = r#"see ./index.nct
use std/string.String
struct RecordingWriter {
    output: String
    writes: usize
    failure_at: usize
}
construct RecordingWriter {
    func accepting(): Self {
        return RecordingWriter { output: String.empty(), writes: 0, failure_at: 1000000 }
    }
    func failing_after(write_count: usize): Self {
        return RecordingWriter { output: String.empty(), writes: 0, failure_at: write_count }
    }
}
instance RecordingWriter {
    method &+self.write(bytes: &[u8]): void! {
        if self.writes >= self.failure_at {
            return error.new("test.destination", "destination rejected line bytes")
        }
        self.writes += 1
        self.output.try_push_utf8(bytes)?
        return
    }
    method &self.text(): &str { return &self.output as &str }
}
test line_adapter_preserves_exact_and_empty_lines {
    var writer = RecordingWriter.accepting()
    writer.write_line("alpha")?
    writer.write_line("")?
    if writer.text() != "alpha\n\n" {
        return error.new("test.output", "Writer line adapter changed its exact bytes")
    }
    return
}
test line_adapter_returns_failure_after_observable_prefix {
    var writer = RecordingWriter.failing_after(1)
    writer.write_line("prefix") catch failure {
        if !failure.has_code("test.destination") || writer.text() != "prefix" {
            return error.new("test.failure", "Writer line failure or prefix changed")
        }
        return
    }
    return error.new("test.failure", "Writer line destination failure was not returned")
}
"#;

const MAP_PHASE3_TEST_SOURCE: &str = r#"see ./index.nct

use std/hash.HashState
use std/mem
use std/string.String
use std/vec.Vec

instance CollisionKey {
    operator (&self == other: &Self): bool { return self.id == other.id }
    noalloc method &self.hash_into(state: &+HashState): void { return }
}

instance Marker {
    operator (&self == other: &Self): bool { return true }
    noalloc method &self.hash_into(state: &+HashState): void { return }
}

struct Counter {
    value: i32
}

test literal_replacement_mutation_and_equality {
    var values = Map [1: 1, 2: 2, 1: 3]
    let one: i32 = 1
    if values.len() != 2 || values[&one] != 3 {
        return error.new("std.map.literal", "mapping literal replacement failed")
    }
    let old = values.insert(2, 7) otherwise {
        return error.new("std.map.replace", "replacement returned no old value")
    }
    if old != 2 { return error.new("std.map.replace_value", "wrong old value") }
    values[&one] = 11
    let expected = Map [2: 7, 1: 11]
    if !(values == expected) {
        return error.new("std.map.equality", "equality depended on placement or order")
    }
    let fruit = Map [String "apple": 3, String "orange": 2]
    let apple = String.copy("apple")
    let another = String.copy("apple")
    if !(apple == another) {
        return error.new("std.map.string_equality", "owned string equality failed")
    }
    let borrowed_apple = &apple
    let borrowed_another = &another
    if !(borrowed_apple == borrowed_another) {
        return error.new("std.map.borrowed_string_equality", "borrowed string equality failed")
    }
    if !fruit.contains_key(&apple) {
        return error.new("std.map.string_key", "owned string key lookup failed")
    }

    var labels = Map [1: String "first"]
    let old_label = labels.insert(1, String "next") otherwise {
        return error.new("std.map.owned_replace", "owned replacement returned no old value")
    }
    if !(old_label == String "first") {
        return error.new("std.map.owned_replace_value", "owned replacement returned wrong value")
    }
    let removed_label = labels.remove(&one) otherwise {
        return error.new("std.map.owned_remove", "owned removal returned no value")
    }
    if !(removed_label == String "next") {
        return error.new("std.map.owned_remove_value", "owned removal returned wrong value")
    }

    var reusable: Map<i32, i32> = Map.with_capacity(4)
    let retained_capacity = reusable.capacity()
    let _ = reusable.insert(1, 9)
    reusable.clear()
    if !reusable.is_empty() || reusable.capacity() != retained_capacity {
        return error.new("std.map.clear", "clear discarded retained capacity")
    }
    return
}

test zero_sized_keys_and_values_preserve_logical_state {
    var values: Map<Marker, Marker> = Map.empty()
    let _ = values.insert(Marker {}, Marker {})
    let replaced = values.insert(Marker {}, Marker {}) otherwise {
        return error.new("std.map.zst_replace", "equal zero-sized key did not replace")
    }
    if values.len() != 1 {
        return error.new("std.map.zst_len", "zero-sized replacement changed length")
    }
    let key = Marker {}
    let _ = values.remove(&key) otherwise {
        return error.new("std.map.zst_remove", "zero-sized entry was absent")
    }
    if !values.is_empty() {
        return error.new("std.map.zst_empty", "zero-sized removal retained an entry")
    }
    return
}

test collisions_growth_and_swap_removal_preserve_lookup {
    var values: Map<CollisionKey, i32> = Map.empty()
    var id: i32 = 0
    while id < 48 {
        let _ = values.insert(CollisionKey { id: id }, id * 3)
        id += 1
    }
    if values.len() != 48 || values.capacity() < 48 {
        return error.new("std.map.growth", "growth lost length or capacity")
    }
    id = 0
    while id < 48 {
        let key = CollisionKey { id: id }
        if values[&key] != id * 3 {
            return error.new("std.map.collision_lookup", "collision lookup failed")
        }
        id += 1
    }
    id = 0
    while id < 32 {
        let key = CollisionKey { id: id }
        let removed = values.remove(&key) otherwise {
            return error.new("std.map.remove", "existing collision key was absent")
        }
        if removed != id * 3 { return error.new("std.map.remove_value", "wrong removed value") }
        id += 2
    }
    id = 1
    while id < 48 {
        let key = CollisionKey { id: id }
        if !values.contains_key(&key) || values[&key] != id * 3 {
            return error.new("std.map.swap_index", "swap removal left a stale bucket")
        }
        id += 2
    }
    return
}

test recoverable_capacity_overflow_is_semantically_atomic {
    var allocator = mem.page_try_allocator()
    var built: Map<i32, i32> = Map.try_from_entries(&+allocator, 1: 10, 2: 20)?
    let built_key: i32 = 2
    if built[&built_key] != 20 {
        return error.new("std.map.try_entries", "recoverable keyed pack construction failed")
    }
    var values: Map<i32, i32> = Map.try_with_capacity(&+allocator, 2)?
    let _ = values.try_insert(7, 70)?
    let previous_capacity = values.capacity()
    let key: i32 = 7
    let maximum: usize = 18446744073709551615
    values.try_reserve(maximum) catch failure {
        if !failure.has_code("std.mem.capacity_overflow") {
            return error.new("std.map.reserve_error", "overflowing reserve returned the wrong error")
        }
        if values.len() != 1 || values.capacity() != previous_capacity {
            return error.new("std.map.failed_reserve", "failed reserve changed state")
        }
        if values[&key] != 70 {
            return error.new("std.map.failed_reserve_value", "failed reserve changed an entry")
        }
        return
    }
    return error.new("std.map.overflow_accepted", "overflowing reserve succeeded")
}

test shared_collection_capacity_and_bounds_errors_are_stable {
    var allocator = mem.page_try_allocator()
    let maximum: usize = 18446744073709551615

    var values: Vec<i32> = Vec.try_with_capacity(&+allocator, 1)?
    values.try_push(7)?
    var vec_overflow_failed = false
    values.try_reserve(maximum) catch failure {
        if !failure.has_code("std.mem.capacity_overflow") {
            return error.new("std.vec.reserve_error", "Vec returned a representation-specific capacity error")
        }
        vec_overflow_failed = true
    }
    if !vec_overflow_failed {
        return error.new("std.vec.overflow_accepted", "overflowing Vec reserve succeeded")
    }
    var vec_bounds_failed = false
    values.try_insert(2, 9) catch failure {
        if !failure.has_code("std.vec.index_out_of_bounds") {
            return error.new("std.vec.insert_error", "Vec returned the wrong insertion error")
        }
        vec_bounds_failed = true
    }
    if !vec_bounds_failed {
        return error.new("std.vec.bounds_accepted", "out-of-bounds Vec insertion succeeded")
    }

    var text = String.try_copy(&+allocator, "x")?
    text.try_reserve(maximum) catch failure {
        if !failure.has_code("std.mem.capacity_overflow") {
            return error.new("std.string.reserve_error", "String returned a representation-specific capacity error")
        }
        return
    }
    return error.new("std.string.overflow_accepted", "overflowing String reserve succeeded")
}

test map_iteration_modes_are_semantic_and_exact {
    var values = Map [1: Counter { value: 10 }, 2: Counter { value: 20 }, 3: Counter { value: 30 }]
    var readonly_count: usize = 0
    var readonly_total: i32 = 0
    for entry in &values {
        readonly_count += 1
        readonly_total += entry.value.value
    }
    if readonly_count != values.len() || readonly_total != 60 {
        return error.new("std.map.readonly_iteration", "readonly iteration lost an entry")
    }

    for entry in &+values {
        entry.value.value += 5
    }
    let one: i32 = 1
    let three: i32 = 3
    if values[&one].value != 15 || values[&three].value != 35 {
        return error.new("std.map.mutable_iteration", "mutable iteration did not update values")
    }
    return
}

test owning_map_iteration_transfers_entries_once {
    let values = Map [1: String "one", 2: String "two", 3: String "three"]
    var seen: usize = 0
    for entry in move values {
        if entry.key == 1 && entry.value == String "one" { seen += 1 }
        if entry.key == 2 && entry.value == String "two" { seen += 1 }
        if entry.key == 3 && entry.value == String "three" { seen += 1 }
    }
    if seen != 3 {
        return error.new("std.map.owning_iteration", "owning iteration lost an entry")
    }
    return
}

test abandoned_owning_iteration_drops_the_remaining_table {
    let values = Map [1: String "one", 2: String "two", 3: String "three"]
    var yielded: usize = 0
    for entry in move values {
        let _ = entry.key
        yielded += 1
        break
    }
    if yielded != 1 {
        return error.new("std.map.owning_cleanup", "owning iteration did not yield once")
    }
    return
}

test set_shares_membership_storage_and_iteration {
    var values = Set [1, 2, 1, 3]
    if values.len() != 3 || values.insert(2) || !values.insert(4) {
        return error.new("std.set.insert", "Set duplicate semantics are incorrect")
    }
    let two: i32 = 2
    if !values.contains(&two) || !values.remove(&two) || values.contains(&two) {
        return error.new("std.set.remove", "Set removal is incorrect")
    }
    let expected = Set [4, 3, 1]
    if !(values == expected) {
        return error.new("std.set.equality", "Set equality depended on placement or order")
    }

    var readonly_count: usize = 0
    for item in &values {
        let _ = item
        readonly_count += 1
    }
    var owned_total: i32 = 0
    for item in move values { owned_total += item }
    if readonly_count != 3 || owned_total != 8 {
        return error.new("std.set.iteration", "Set iteration lost a value")
    }
    return
}

"#;

struct TestDiscoveredUnit {
    computation: nocter_compiler_computation::CompilerComputation,
    discovered: nocter_compiler_computation::CompilerDiscoveredUnit,
}

impl std::ops::Deref for TestDiscoveredUnit {
    type Target = DiscoveredUnit;

    fn deref(&self) -> &Self::Target {
        self.discovered.unit()
    }
}

fn discover(request: DiscoveryRequest) -> Result<TestDiscoveredUnit, Box<dyn std::error::Error>> {
    let mut computation = nocter_compiler_computation::CompilerComputation::new();
    let revision = computation.advance_sources(request.source_overlay(), 0)?;
    let discovered = computation.discover(&revision, request)?;
    Ok(TestDiscoveredUnit {
        computation,
        discovered,
    })
}

fn analyze_for_test(mut unit: TestDiscoveredUnit) -> AnalyzedUnit {
    let product = unit.computation.analyze(&unit.discovered).unwrap();
    nocter_session::analyze_unit_from_query(&product).unwrap()
}

fn compile_for_test(unit: TestDiscoveredUnit) -> CompiledTarget {
    analyze_for_test(unit).into_compilation_result().unwrap()
}

const DIRECTORY_RECORD_TEST_SOURCE: &[u8] = br#"see ./directory.nct

use /internal/os/darwin
use /internal/ptr as internal_ptr
use /mem
use /path.Utf8Path
use /ptr

func test_reader(
    record_len: u8,
    name_len: u8,
    name_byte: u8,
    terminator: u8,
): ReadDir! {
    var allocator = mem.page_try_allocator()
    var buffer = allocator.try_alloc(64, 8)?
    let address = ptr.addr(buffer.bytes_mut().ptr())
    let type_pointer: *u8 = internal_ptr.from_addr(address)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_INODE_OFFSET, 1)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_RECORD_LENGTH_OFFSET, record_len)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_RECORD_LENGTH_OFFSET + 1, 0)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_NAME_LENGTH_OFFSET, name_len)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_NAME_LENGTH_OFFSET + 1, 0)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_TYPE_OFFSET, darwin.DIRENT_TYPE_REGULAR)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_NAME_OFFSET, name_byte)
    internal_ptr.store_u8_to_ptr(type_pointer, darwin.DIRENT_NAME_OFFSET + name_len as usize, terminator)
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
    let compiled = compile_for_test(unit);

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

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    assert!(!image.image().bytes().is_empty());
}

#[test]
fn standard_text_transformations_cross_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/ptr

func main(): i32 {
    let text: &str = " \tNocter\r\n"
    if text.trim_ascii_start() != "Nocter\r\n" { return 1 }
    if text.trim_ascii_end() != " \tNocter" { return 2 }
    if text.trim_ascii() != "Nocter" { return 3 }

    let whitespace: &str = "\t \r\n"
    let empty_start = whitespace.trim_ascii_start()
    let empty_end = whitespace.trim_ascii_end()
    let empty_both = whitespace.trim_ascii()
    if empty_start.len() != 0 || empty_end.len() != 0 || empty_both.len() != 0 { return 4 }
    let whitespace_end = ptr.addr(whitespace.ptr()) + whitespace.len()
    if ptr.addr(empty_start.ptr()) != whitespace_end
        || ptr.addr(empty_end.ptr()) != whitespace_end
        || ptr.addr(empty_both.ptr()) != whitespace_end { return 5 }

    let repeated = "é".repeat(3)
    if (&repeated as &str) != "ééé" { return 6 }
    let replaced = "aaaa/é".replace_all("aa", "b") catch _ { return 7 }
    if (&replaced as &str) != "bb/é" { return 8 }
    let _invalid = "x".replace_all("", "y") catch failure {
        if failure.has_code("std.str.empty_pattern") { return 0 }
        return 9
    }
    return 10
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

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "text-transformations");
}

#[test]
fn standard_directory_stream_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/fs.FileType
use std/fs

func open_and_drop(): void! {
    let stream = fs.read_dir(".")?
    return
}

func open_fails_with(path: &str, code: &str): bool {
    let _stream = fs.read_dir(path) catch failure {
        return failure.has_code(code)
    }
    return false
}

func inspect_directory(): i32! {
    var stream = fs.read_dir(".")?
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

    var closed = fs.read_dir(".")?
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

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_directory_stream(image.image(), &package_root.0, 42);
}

#[test]
fn public_path_and_directory_lifecycle_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/fs
use std/path.Utf8Path

func main(): i32! {
    let target = Utf8Path.new("workspace/cache/items.json")?
    let parent = target.parent() otherwise { return 1 }
    fs.create_dir_all(parent)?
    fs.write_text(&target, "value")?

    let file_name = target.file_name() otherwise { return 2 }
    let stem = target.file_stem() otherwise { return 3 }
    let extension = target.extension() otherwise { return 4 }
    if file_name != "items.json" { return 5 }
    if stem != "items" { return 6 }
    if extension != "json" { return 7 }

    fs.remove_file(&target)?
    fs.remove_dir("workspace/cache")?
    fs.remove_dir("workspace")?

    var dangling_rejected = false
    fs.create_dir_all("dangling-root/link/child") catch failure {
        dangling_rejected = failure.has_code("std.io.not_directory")
    }
    if !dangling_rejected { return 8 }

    var symlink_remove_rejected = false
    fs.remove_dir("dangling-root/link") catch failure {
        symlink_remove_rejected = failure.has_code("std.io.not_directory")
    }
    if !symlink_remove_rejected { return 9 }
    fs.remove_file("dangling-root/link")?
    fs.remove_dir("dangling-root")?

    fs.create_dir_all("linked-root/child")?
    fs.remove_dir("linked-root/child")?
    fs.remove_file("linked-root")?
    fs.remove_dir("real-root")?
    return 0
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
        "public path and directory fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    prepare_path_directory_fixture(&package_root.0);
    execute_native_test(image.image(), &package_root.0, "public-path-directory");
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

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_streaming_lines(image.image(), &package_root.0, 42);
}

#[test]
fn standard_input_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/io.Reader
use std/io
use std/vec.Vec

func drop_stdin_wrapper(): void {
    let wrapper = io.stdin()
    return
}

func main(): i32! {
    var input = io.stdin()
    let bytes = input.read_to_end()?
    if bytes.len() != 7 { return 1 }
    if bytes[0] != 97 || bytes[1] != 108 || bytes[2] != 112 { return 2 }
    if bytes[3] != 104 || bytes[4] != 97 || bytes[5] != 10 || bytes[6] != 255 { return 3 }

    input.close()
    var empty: Vec<u8> = Vec.empty()
    let _ = input.read(&+empty) catch closed_failure {
        if !closed_failure.has_code("std.io.closed") { return 4 }

        var after_close = io.stdin()
        if after_close.read(&+empty)? != 0 { return 5 }
        after_close.close()

        drop_stdin_wrapper()
        var after_drop = io.stdin()
        if after_drop.read(&+empty)? != 0 { return 6 }
        return 42
    }
    return 7
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
        "standard input fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_standard_input(image.image(), &package_root.0, b"alpha\n\xff", 42);
}

#[test]
fn standard_buffered_input_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/io
use std/io/buffer.BufReader
use std/string.String

func main(): i32! {
    var input = BufReader.with_capacity(io.stdin(), 3)
    var line = String.with_capacity(64)
    let original_capacity = line.capacity()

    if !input.read_line_into(&+line)? || (&line as &str) != "" { return 1 }
    if !input.read_line_into(&+line)? || (&line as &str) != "alpha" { return 2 }
    if !input.read_line_into(&+line)? || (&line as &str) != "lone\rbeta" { return 3 }
    if !input.read_line_into(&+line)? || (&line as &str) != "😀 split" { return 4 }
    let final_line = input.read_line()? otherwise { return 5 }
    if (&final_line as &str) != "final" { return 6 }
    if input.read_line_into(&+line)? { return 7 }
    if (&line as &str) != "" { return 8 }
    if line.capacity() != original_capacity { return 9 }
    let _after_eof = input.read_line()? otherwise { return 42 }
    return 10
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
        "buffered standard input fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_standard_input(
        image.image(),
        &package_root.0,
        b"\nalpha\r\nlone\rbeta\n\xf0\x9f\x98\x80 split\nfinal",
        42,
    );
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

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "collection-ordering");
}

#[test]
fn standard_filesystem_contract_crosses_native_tests() {
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

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard filesystem tests failed native compilation")
    };
    assert_eq!(cases.len(), 4);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_path_lexical_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"path\", module: \"./path\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["path"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard path tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_hash_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"hash\", module: \"./hash\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["hash"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard hash tests failed native compilation")
    };
    assert_eq!(cases.len(), 4);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_format_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"format\", module: \"./fmt\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["fmt"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard format tests failed native compilation")
    };
    assert_eq!(cases.len(), 1);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_io_descriptor_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
        panic!("public Writer line tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_num_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
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
    assert_eq!(cases.len(), 8);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_time_value_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
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
    assert_eq!(cases.len(), 8);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn integer_text_propagates_recoverable_allocator_failure() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");

    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"numeric\", module: \"./num\" }\n");

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
    let num_failure_tests = concat!(
        "see ./index.nct\n",
        "use /mem\n",
        "test recoverable_integer_text_propagates_allocator_failure {\n",
        "    var allocator = mem.failing_try_allocator_for_test()\n",
        "    let value: i64 = -9223372036854775808\n",
        "    let _text = value.try_to_string(&+allocator) catch failure {\n",
        "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
        "        return error.new(\"std.num.allocator\", \"wrong allocator failure\")\n",
        "    }\n",
        "    return error.new(\"std.num.allocator\", \"invalid allocator succeeded\")\n",
        "}\n",
    );

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
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("allocator failure numeric tests failed native compilation")
    };
    assert_eq!(cases.len(), 9);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
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
    assert_eq!(case_count, 19);
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
        panic!("standard JSON Writer tests failed native compilation")
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

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert!(!analysis.diagnostics().is_empty());
    let semantic = analysis.semantic_evidence().unwrap();
    assert!(!semantic.graph().declarations().callables().is_empty());
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| matches!(
                semantic.body_names(body),
                Some(nocter_session::SemanticBodyNamesView::Available(_))
            ))
    );
    assert!(!semantic.source_index().is_empty());
    let primary = analysis.diagnostics()[0].primary();
    let interruption = semantic
        .interruption_overlapping(primary.source(), primary.span().range())
        .unwrap();
    assert_eq!(
        interruption.origin().span(),
        analysis.diagnostics().first().unwrap().primary().span()
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

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert_eq!(analysis.diagnostics()[0].code(), "E0340");
    let semantic = analysis.semantic_evidence().unwrap();
    assert!(!semantic.graph().declarations().callables().is_empty());
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| semantic.body_names(body).is_some())
    );
    assert!(!semantic.source_index().is_empty());
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

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert_eq!(analysis.diagnostics()[0].code(), "E0350");
    let declarations = analysis.semantic_evidence().unwrap();
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
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    let semantic = analysis.semantic_evidence().expect("typed syntax recovery");
    let diagnostic = analysis.unit().syntax_diagnostics()[0].primary();
    let recovery = semantic
        .interruption_overlapping(diagnostic.source(), diagnostic.span().range())
        .expect("expected body evidence");
    assert!(matches!(
        recovery.kind(),
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
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
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
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0350")
    );
    let semantic = analysis.semantic_evidence().expect("declaration analysis");
    let declarations = semantic;
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
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0340")
    );
    let semantic = analysis.semantic_evidence().expect("name analysis");
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| semantic.body_names(body).is_some())
    );
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
        let target_program = compile_for_test(unit);
        compile_native_image(ExecutableCompileRequest::only(target_program))
            .unwrap_or_else(|error| panic!("{} failed compilation: {error:?}", source.display()));
    }
}

#[test]
fn every_public_package_example_crosses_the_complete_target_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let examples_root = compiler_root.join("../../examples");
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
        let target_program = compile_for_test(unit);
        let target = compile_native_image(ExecutableCompileRequest::named(
            target_program,
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
        let target = compile_for_test(unit);
        compile_native_images(NativeImageSetCompileRequest::all(target)).unwrap()
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

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
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
    let target = compile_for_test(unit);
    let selected =
        compile_native_tests(NativeTestCompileRequest::case(target, "unit", "second")).unwrap();
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
    package_graph_with_overlay(packages, SourceOverlay::empty())
}

fn package_graph_with_overlay(
    packages: Vec<ResolvedPackageSpec>,
    overlay: SourceOverlay,
) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load_with_root_catalog(
        packages,
        nocter_package::PackageRootCatalog::new(overlay),
        &mut nocter_syntax::DirectSourceSyntax,
    )
    .unwrap()
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
fn execute_standard_input(image: &NativeImage, root: &Path, input: &[u8], expected: i32) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let executable = root.join("standard-input");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new(&executable)
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let status = child.wait().unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "standard input executable exited with {status:?}"
    );
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn prepare_path_directory_fixture(root: &Path) {
    use std::os::unix::fs::symlink;

    fs::create_dir(root.join("dangling-root")).unwrap();
    symlink("missing", root.join("dangling-root/link")).unwrap();
    fs::create_dir(root.join("real-root")).unwrap();
    symlink("real-root", root.join("linked-root")).unwrap();
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_directory_stream(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_streaming_lines(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_standard_input(_image: &NativeImage, _root: &Path, _input: &[u8], _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_native_test(_image: &NativeImage, _root: &Path, _name: &str) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn prepare_path_directory_fixture(_root: &Path) {}
