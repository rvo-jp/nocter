use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveredUnit, DiscoveryRequest};
use nocter_filesystem::{SourceOverlay, SourceOverride};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::{PrimitiveRole, RuntimeStorageRole};
use nocter_standard_profile::bundled_standard_toolchain;

use super::{
    NativeImage, NativeImageSetCompileRequest, NativeTestCompileRequest, NativeTestTargetOutcome,
    compile_native_image, compile_native_images, compile_native_tests,
};
use nocter_session::{AnalyzedUnit, AnalyzedUnitStatus, CompiledTarget, ExecutableCompileRequest};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

const JSON_WRITER_CONTRACT_TEST_SOURCE: &str = r#"//! Public JSON BlockingWriter contract tests.
#package: { name: "json-writer-tests", version: "0.0.0", }
#test: { name: "writer", module: "." }
use std/io.BlockingWriter
use std/string.String
see ./implementation.nct
pub struct RecordingWriter
construct RecordingWriter {
    pub func accepting(): Self
    pub func failing_after(write_count: usize): Self
}
instance RecordingWriter {
    impl BlockingWriter
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
    method &+self.write_blocking(bytes: &[u8]): void! {
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
        return error.new("test.output", "BlockingWriter spelling diverged from String generation")
    }
    return
}
test try_write_uses_the_selected_traversal_allocator {
    let value = json.parse("[null,true,-0]")?
    var allocator = mem.page_try_allocator()
    var writer = RecordingWriter.accepting()
    json.try_write(&+allocator, &+writer, &value)?
    if writer.text() != "[null,true,-0]" {
        return error.new("test.output", "recoverable BlockingWriter spelling changed")
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

const IO_WRITER_CONTRACT_TEST_SOURCE: &str = r#"//! Public BlockingWriter line-adapter tests.
#package: { name: "io-writer-tests", version: "0.0.0", }
#test: { name: "writer", module: "." }
use std/io.BlockingWriter
use std/string.String
see ./implementation.nct
pub struct RecordingWriter
construct RecordingWriter {
    pub func accepting(): Self
    pub func failing_after(write_count: usize): Self
}
instance RecordingWriter {
    impl BlockingWriter
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
    method &+self.write_blocking(bytes: &[u8]): void! {
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
    writer.write_line_blocking("alpha")?
    writer.write_line_blocking("")?
    if writer.text() != "alpha\n\n" {
        return error.new("test.output", "BlockingWriter line adapter changed its exact bytes")
    }
    return
}
test line_adapter_returns_failure_after_observable_prefix {
    var writer = RecordingWriter.failing_after(1)
    writer.write_line_blocking("prefix") catch failure {
        if !failure.has_code("test.destination") || writer.text() != "prefix" {
            return error.new("test.failure", "BlockingWriter line failure or prefix changed")
        }
        return
    }
    return error.new("test.failure", "BlockingWriter line destination failure was not returned")
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn compile_single_file_native_source(
    package_root: &TempPackage,
    standard_root: &Path,
    source: &str,
) -> NativeImage {
    package_root.source("main.nct", source);
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let compiled = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();
    compiled.into_parts().0
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn byte_vector_source(bytes: &[u8]) -> String {
    let elements = bytes
        .iter()
        .map(|byte| format!("u8.truncate({byte})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Vec [{elements}]")
}

#[test]
fn scalar_floating_values_cross_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/vec.Vec\n\
         \n\
         const COMPILED_SUM: f64 = 0.1 + 0.2\n\
         const COMPILED_NAN: f64 = 0.0 / 0.0\n\
         const COMPILED_TINY: f64 = 5e-324\n\
         \n\
         func identity(value: f64): f64 { value }\n\
         func sum9(\n\
             a: f64, b: f64, c: f64, d: f64, e: f64,\n\
             f: f64, g: f64, h: f64, i: f64,\n\
         ): f64 { a + b + c + d + e + f + g + h + i }\n\
         func mixed9(\n\
             n0: i32, f0: f64, n1: i32, f1: f64, n2: i32, f2: f64,\n\
             n3: i32, f3: f64, n4: i32, f4: f64, n5: i32, f5: f64,\n\
             n6: i32, f6: f64, n7: i32, f7: f64, n8: i32, f8: f64,\n\
         ): f64 { n8 as f64 + f8 }\n\
         func narrow(value: f32): f32 { -(value * 2.0f32) }\n\
         func main(): i32 {\n\
             let retained = 1.25\n\
             let returned = identity(0.75)\n\
             let stacked = sum9(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)\n\
             if retained + returned != 2.0 { return 1 }\n\
             if stacked != 9.0 { return 2 }\n\
             if narrow(0.5f32) != -1.0f32 { return 3 }\n\
             if !(returned < retained) { return 4 }\n\
             var values = Vec [1.5, 2.5]\n\
             let popped = values.pop() otherwise { return 5 }\n\
             if popped != 2.5 { return 6 }\n\
             let signed: i32 = -7\n\
             if signed as f64 != -7.0 { return 7 }\n\
             let unsigned: u16 = 9\n\
             if unsigned as f32 != 9.0f32 { return 8 }\n\
             let widened = 1.5f32 as f64\n\
             if widened != 1.5 { return 9 }\n\
             let nan = 0.0 / 0.0\n\
             if nan <= 1.0 { return 10 }\n\
             if nan >= 1.0 { return 11 }\n\
             if nan == nan { return 12 }\n\
             if !(nan != nan) { return 13 }\n\
             if !(1.0 <= 1.0) || !(1.0 >= 1.0) { return 14 }\n\
             if COMPILED_SUM != 0.30000000000000004 { return 15 }\n\
             if COMPILED_NAN <= 1.0 || COMPILED_NAN >= 1.0 { return 16 }\n\
             let mixed = mixed9(\n\
                 0, 0.0, 1, 1.0, 2, 2.0, 3, 3.0, 4, 4.0,\n\
                 5, 5.0, 6, 6.0, 7, 7.0, 8, 8.0,\n\
             )\n\
             if mixed != 16.0 { return 17 }\n\
             let tiny64 = 5e-324\n\
             if !(tiny64 > 0.0) || tiny64 / 2.0 != 0.0 { return 18 }\n\
             if COMPILED_TINY != tiny64 { return 19 }\n\
             let tiny32 = 1e-45f32\n\
             if !(tiny32 > 0.0f32) || tiny32 / 2.0f32 != 0.0f32 { return 20 }\n\
             let positive_infinity = 1.0 / 0.0\n\
             let negative_infinity = 1.0 / -0.0\n\
             if !(positive_infinity > 1.7976931348623157e308) { return 21 }\n\
             if !(negative_infinity < -1.7976931348623157e308) { return 22 }\n\
             if -0.0 != 0.0 { return 23 }\n\
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
    execute_native_status(image.image(), &package_root.0, "floating", 0);
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

const COLLECTION_ORDERING_TEST_SOURCE: &str = r#"use std/order.TotalOrder
use std/string.String
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
    impl TotalOrder

    operator (&self == other: &Self): bool {
        return self.key == other.key
    }

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

const PROVIDER_ASYNC_STREAM_TEST_MAIN: &str = r#"
async func main(): i32 {
    var listener = match await listen_stream_async(NetworkAddress.ipv4([127, 0, 0, 1], 0)) {
        StreamListenerAttempt.ready(ready_listener) { move ready_listener }
        StreamListenerAttempt.failed(_) { return 1 }
    }
    let address = match listener.local_address() {
        AddressAttempt.ready(value) { value }
        AddressAttempt.failed(_) { return 2 }
    }
    var client = match await connect_stream_async_with_timeout(
        address,
        Duration.from_seconds(1),
    ) {
        StreamConnectionAttempt.ready(connection) { move connection }
        StreamConnectionAttempt.failed(_) { return 3 }
    }
    var server = match await listener.accept_async() {
        StreamListenerAcceptAttempt.ready(connection, _) { move connection }
        StreamListenerAcceptAttempt.failed(_) { return 4 }
    }
    match await client.write_async_with_timeout(
        "ping".bytes(),
        Duration.from_seconds(1),
    ) {
        UnitAttempt.ready {}
        UnitAttempt.failed(_) { return 5 }
    }
    var request: Vec<u8> = Vec [
        u8.truncate(0), u8.truncate(0), u8.truncate(0), u8.truncate(0),
    ]
    let request_len = match await server.read_async(&+request) {
        TransferAttempt.ready(count) { count }
        TransferAttempt.failed(_) { return 6 }
    }
    if request_len != 4 || request[0] != 112 || request[3] != 103 { return 7 }
    match await server.write_async("pong".bytes()) {
        UnitAttempt.ready {}
        UnitAttempt.failed(_) { return 8 }
    }
    var response: Vec<u8> = Vec [
        u8.truncate(0), u8.truncate(0), u8.truncate(0), u8.truncate(0),
    ]
    let response_len = match await client.read_async_with_timeout(
        &+response,
        Duration.from_seconds(1),
    ) {
        TransferAttempt.ready(count) { count }
        TransferAttempt.failed(_) { return 9 }
    }
    if response_len != 4 || response[0] != 112 || response[3] != 103 { return 10 }
    var waiting: Vec<u8> = Vec [u8.truncate(0)]
    match await client.read_async_with_timeout(
        &+waiting,
        Duration.from_milliseconds(2),
    ) {
        TransferAttempt.ready(_) { return 11 }
        TransferAttempt.failed(failure) {
            match move failure {
                NetworkFailure.timed_out {}
                _ { return 12 }
            }
        }
    }
    match await client.write_async("closed".bytes()) {
        UnitAttempt.failed(failure) {
            match move failure {
                NetworkFailure.closed { return 0 }
                _ { return 13 }
            }
        }
        _ { return 14 }
    }
}
"#;

const PROVIDER_LISTENER_POLICY_TEST_MAIN: &str = r"
async func main(): i32 {
    var listener = match await listen_stream_async(NetworkAddress.ipv4([127, 0, 0, 1], 0)) {
        StreamListenerAttempt.ready(value) { move value }
        StreamListenerAttempt.failed(_) { return 1 }
    }
    let address = match listener.local_address() {
        AddressAttempt.ready(value) { value }
        AddressAttempt.failed(_) { return 2 }
    }
    if address.port == 0 { return 3 }
    match await listener.accept_async_with_timeout(Duration.from_milliseconds(2)) {
        StreamListenerAcceptAttempt.failed(failure) {
            match move failure {
                NetworkFailure.timed_out {}
                _ { return 4 }
            }
        }
        StreamListenerAcceptAttempt.ready(_, _) { return 5 }
    }
    var client = match await connect_stream_async_with_timeout(
        address,
        Duration.from_seconds(1),
    ) {
        StreamConnectionAttempt.ready(connection) { move connection }
        StreamConnectionAttempt.failed(_) { return 6 }
    }
    let accepted = match await listener.accept_async_with_timeout(Duration.from_seconds(1)) {
        StreamListenerAcceptAttempt.ready(connection, peer) { (move connection, peer) }
        StreamListenerAcceptAttempt.failed(_) { return 7 }
    }
    var server = move accepted.0
    if accepted.1.port == 0 { return 8 }
    server.close()
    var end: Vec<u8> = Vec [u8.truncate(0)]
    match await client.read_async_with_timeout(&+end, Duration.from_seconds(1)) {
        TransferAttempt.ready(count) {
            if count != 0 { return 10 }
        }
        TransferAttempt.failed(_) { return 11 }
    }
    client.close()
    listener.close()
    listener.close()
    match await listener.accept_async() {
        StreamListenerAcceptAttempt.failed(failure) {
            match move failure {
                NetworkFailure.closed { return 0 }
                _ { return 12 }
            }
        }
        StreamListenerAcceptAttempt.ready(_, _) { return 13 }
    }
}
";

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
    assert!(
        compiled
            .program()
            .toolchain()
            .runtime_storage()
            .declaration(RuntimeStorageRole::NetworkOwner)
            .is_some()
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
fn standard_unicode_lookup_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"unicode\", module: \"./internal/unicode\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["internal", "unicode"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard Unicode tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_subprocess_contract_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let helper = package_root.0.join("subprocess-helper");
    let missing = package_root.0.join("missing-executable");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.{{Command, ExitStatus}}
use std/string.String

noalloc func has_signal(status: ExitStatus): bool {{
    let _ = status.signal() otherwise {{ return false }}
    return true
}}

blocking func main(): i32 {{
    var path = String.copy("{}")
    var first = String.copy("alpha beta")
    var command = Command.new(&path as &str) catch _ {{ return 1 }}
    command.arg(&first as &str) catch _ {{ return 2 }}
    command.arg("") catch _ {{ return 3 }}
    path.clear()
    first.clear()

    let status = command.status() catch _ {{ return 4 }}
    let code = status.code() otherwise {{ return 5 }}
    if status.success() || code != 7 || has_signal(status) {{ return 6 }}

    let missing = Command.new("{}") catch _ {{ return 8 }}
    let _status = missing.status() catch failure {{
        if failure.has_code("std.process.not_found") {{ return 0 }}
        return 9
    }}
    return 10
}}
"#,
            helper.display(),
            missing.display(),
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
    execute_subprocess_contract(image.image(), &package_root.0);
}

#[test]
fn standard_subprocess_output_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let helper = package_root.0.join("capture-helper");
    let empty = package_root.0.join("empty-capture-helper");
    let text = package_root.0.join("text-capture-helper");
    let signaled = package_root.0.join("signal-capture-helper");
    let missing = package_root.0.join("missing-capture-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.Command

noalloc func matches_stream(
    bytes: &[u8],
    repeated: u8,
    repeated_len: usize,
    first_tail: u8,
    second_tail: u8,
): bool {{
    if bytes.len() != repeated_len + 2 {{ return false }}
    var index: usize = 0
    while index < repeated_len {{
        if bytes[index] != repeated {{ return false }}
        index += 1
    }}
    return bytes[repeated_len] == first_tail
        && bytes[repeated_len + 1] == second_tail
}}

blocking func main(): i32 {{
    let command = Command.new("{}") catch _ {{ return 1 }}
    let output = command.output() catch _ {{ return 2 }}
    let code = output.status.code() otherwise {{ return 3 }}
    if output.status.success() || code != 23 {{ return 4 }}

    let stdout: &[u8] = &output.stdout as &[u8]
    let stderr: &[u8] = &output.stderr as &[u8]
    if !matches_stream(stdout, 79, 262144, 0, 255) {{ return 5 }}
    if !matches_stream(stderr, 69, 262144, 0, 254) {{ return 6 }}

    let empty = Command.new("{}") catch _ {{ return 7 }}
    let empty_output = empty.output() catch _ {{ return 8 }}
    if !empty_output.status.success() || empty_output.stdout.len() != 0
        || empty_output.stderr.len() != 0 {{ return 9 }}

    let text = Command.new("{}") catch _ {{ return 10 }}
    let text_output = text.output() catch _ {{ return 11 }}
    if !text_output.status.success() || text_output.stdout.len() != 6
        || text_output.stdout[0] != 104 || text_output.stdout[1] != 101
        || text_output.stdout[2] != 108 || text_output.stdout[3] != 108
        || text_output.stdout[4] != 111 || text_output.stdout[5] != 10
        || text_output.stderr.len() != 8 || text_output.stderr[0] != 119
        || text_output.stderr[1] != 97 || text_output.stderr[2] != 114
        || text_output.stderr[3] != 110 || text_output.stderr[4] != 105
        || text_output.stderr[5] != 110 || text_output.stderr[6] != 103
        || text_output.stderr[7] != 10 {{ return 12 }}

    let signaled = Command.new("{}") catch _ {{ return 13 }}
    let signal_output = signaled.output() catch _ {{ return 14 }}
    let signal = signal_output.status.signal() otherwise {{ return 15 }}
    if signal != 15 || signal_output.stdout.len() != 10
        || signal_output.stdout[0] != 115 || signal_output.stdout[1] != 105
        || signal_output.stdout[2] != 103 || signal_output.stdout[3] != 110
        || signal_output.stdout[4] != 97 || signal_output.stdout[5] != 108
        || signal_output.stdout[6] != 45 || signal_output.stdout[7] != 111
        || signal_output.stdout[8] != 117 || signal_output.stdout[9] != 116
        || signal_output.stderr.len() != 12 || signal_output.stderr[0] != 115
        || signal_output.stderr[1] != 105 || signal_output.stderr[2] != 103
        || signal_output.stderr[3] != 110 || signal_output.stderr[4] != 97
        || signal_output.stderr[5] != 108 || signal_output.stderr[6] != 45
        || signal_output.stderr[7] != 101 || signal_output.stderr[8] != 114
        || signal_output.stderr[9] != 114 || signal_output.stderr[10] != 111
        || signal_output.stderr[11] != 114 {{ return 16 }}

    var attempt: usize = 0
    while attempt < 48 {{
        let repeated = Command.new("{}") catch _ {{ return 17 }}
        let repeated_output = repeated.output() catch _ {{ return 18 }}
        if !repeated_output.status.success() || repeated_output.stdout.len() != 0
            || repeated_output.stderr.len() != 0 {{ return 19 }}
        attempt += 1
    }}

    let missing = Command.new("{}") catch _ {{ return 20 }}
    let _missing_output = missing.output() catch failure {{
        if failure.has_code("std.process.not_found") {{ return 0 }}
        return 21
    }}
    return 22
}}
"#,
            helper.display(),
            empty.display(),
            text.display(),
            signaled.display(),
            empty.display(),
            missing.display(),
        ),
    );
    compile_and_execute_subprocess_output(&package_root.0, &standard_root);
}

const CONFIGURED_SUBPROCESS_HELPERS_SOURCE: &str = r"use std/process.Command
use std/vec.Vec

func repeated(byte: u8, count: usize): Vec<u8> {
    var bytes: Vec<u8> = Vec.with_capacity(count)
    var index: usize = 0
    while index < count {
        bytes.push(byte)
        index += 1
    }
    return move bytes
}

noalloc func range_matches(bytes: &[u8], start: usize, count: usize, byte: u8): bool {
    if start + count > bytes.len() { return false }
    var index: usize = 0
    while index < count {
        if bytes[start + index] != byte { return false }
        index += 1
    }
    return true
}

blocking func status_fails_with(command: Command, code: &str): bool {
    let _status = command.status() catch failure { return failure.has_code(code) }
    return false
}

blocking func output_fails_with(command: Command, code: &str): bool {
    let _output = command.output() catch failure { return failure.has_code(code) }
    return false
}
";

#[test]
fn configured_subprocess_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let workspace = package_root.0.join("configured-workspace");
    let inherited_helper = package_root.0.join("inherited-helper");
    let transfer_helper = package_root.0.join("transfer-helper");
    let empty_helper = package_root.0.join("empty-input-helper");
    let early_close_helper = package_root.0.join("early-close-helper");
    let missing_directory = package_root.0.join("missing-directory");
    package_root.source(
        "main.nct",
        &format!(
            r#"{CONFIGURED_SUBPROCESS_HELPERS_SOURCE}
blocking func main(): i32 {{
    var exact = Command.new("./environment-helper") catch _ {{ return 1 }}
    exact.current_dir("{}") catch _ {{ return 2 }}
    exact.clear_env()
    exact.env("KEEP", "first") catch _ {{ return 3 }}
    exact.env("KEEP", "final=value") catch _ {{ return 4 }}
    exact.env("REMOVE", "present") catch _ {{ return 5 }}
    exact.remove_env("REMOVE") catch _ {{ return 6 }}
    let exact_status = exact.status() catch _ {{ return 7 }}
    if !exact_status.success() {{
        return exact_status.code() otherwise {{ return 8 }}
    }}

    var inherited = Command.new("{}") catch _ {{ return 9 }}
    inherited.env("NOCTER_CHANGED", "child=value") catch _ {{ return 10 }}
    inherited.remove_env("NOCTER_REMOVED") catch _ {{ return 11 }}
    let inherited_status = inherited.status() catch _ {{ return 12 }}
    if !inherited_status.success() {{
        return inherited_status.code() otherwise {{ return 13 }}
    }}

    let input_byte: u8 = 73
    let stdout_byte: u8 = 79
    let stderr_byte: u8 = 69
    let transfer_count: usize = 131072
    let input = repeated(input_byte, transfer_count)
    var transfer = Command.new("{}") catch _ {{ return 14 }}
    transfer.input(&input)
    let output = transfer.output() catch _ {{ return 15 }}
    if !output.status.success() || output.stdout.len() != transfer_count * 2
        || output.stderr.len() != transfer_count {{ return 16 }}
    if !range_matches(&output.stdout, 0, transfer_count, stdout_byte)
        || !range_matches(&output.stdout, transfer_count, transfer_count, input_byte)
        || !range_matches(&output.stderr, 0, transfer_count, stderr_byte) {{ return 17 }}

    let empty: Vec<u8> = Vec.empty()
    var empty_command = Command.new("{}") catch _ {{ return 18 }}
    empty_command.input(&empty)
    let empty_status = empty_command.status() catch _ {{ return 19 }}
    if !empty_status.success() {{ return 20 }}

    let early_bytes = repeated(input_byte, 1048576)
    var early = Command.new("{}") catch _ {{ return 21 }}
    early.input(&early_bytes)
    let early_output = early.output() catch _ {{ return 22 }}
    if !early_output.status.success() || early_output.stdout.len() != 0
        || early_output.stderr.len() != 0 {{ return 23 }}

    var bad_directory = Command.new("./never-executed") catch _ {{ return 24 }}
    bad_directory.current_dir("{}") catch _ {{ return 25 }}
    if !status_fails_with(move bad_directory, "std.process.current_directory_failed") {{ return 26 }}

    var bad_output_directory = Command.new("./never-executed") catch _ {{ return 27 }}
    bad_output_directory.current_dir("{}") catch _ {{ return 28 }}
    if !output_fails_with(move bad_output_directory, "std.process.current_directory_failed") {{
        return 29
    }}
    return 0
}}
"#,
            workspace.display(),
            inherited_helper.display(),
            transfer_helper.display(),
            empty_helper.display(),
            early_close_helper.display(),
            missing_directory.display(),
            missing_directory.display(),
        ),
    );
    compile_and_execute_configured_subprocess(&package_root.0, &standard_root);
}

#[test]
fn standard_process_internal_contracts_cross_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"process\", module: \"./process\" }\n");
    root_source.push_str("#test: { name: \"darwin-pair\", module: \"./internal/os/darwin\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["process"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "os", "darwin"]),
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
            panic!("standard process tests failed native compilation")
        };
        for case in cases {
            case_count += 1;
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 13);
}

#[test]
fn standard_subprocess_failures_and_lifecycle_cross_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let success = package_root.0.join("success-helper");
    let nonzero = package_root.0.join("nonzero-helper");
    let exit_127 = package_root.0.join("exit-127-helper");
    let signaled = package_root.0.join("signal-helper");
    let missing = package_root.0.join("missing-helper");
    let denied = package_root.0.join("denied-helper");
    let invalid = package_root.0.join("invalid-helper");
    let arguments = package_root.0.join("argument-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.{{Command, ExitStatus}}
use std/string.String

noalloc func exited_with(status: ExitStatus, expected: i32): bool {{
    let code = status.code() otherwise {{ return false }}
    let _signal = status.signal() otherwise {{ return code == expected }}
    return false
}}

noalloc func signaled_with(status: ExitStatus, expected: i32): bool {{
    let signal = status.signal() otherwise {{ return false }}
    let _code = status.code() otherwise {{ return signal == expected }}
    return false
}}

blocking func fails_with(command: Command, code: &str): bool {{
    let _status = command.status() catch failure {{ return failure.has_code(code) }}
    return false
}}

blocking func main(): i32 {{
    let success = Command.new("{}") catch _ {{ return 1 }}
    let success_status = success.status() catch _ {{ return 2 }}
    if !success_status.success() || !exited_with(success_status, 0) {{ return 3 }}

    let nonzero = Command.new("{}") catch _ {{ return 4 }}
    let nonzero_status = nonzero.status() catch _ {{ return 5 }}
    if nonzero_status.success() || !exited_with(nonzero_status, 23) {{ return 6 }}

    let ordinary_127 = Command.new("{}") catch _ {{ return 7 }}
    let ordinary_127_status = ordinary_127.status() catch _ {{ return 8 }}
    if ordinary_127_status.success() || !exited_with(ordinary_127_status, 127) {{ return 9 }}

    let signaled = Command.new("{}") catch _ {{ return 10 }}
    let signal_status = signaled.status() catch _ {{ return 11 }}
    if signal_status.success() || !signaled_with(signal_status, 15) {{ return 12 }}

    let missing = Command.new("{}") catch _ {{ return 13 }}
    if !fails_with(move missing, "std.process.not_found") {{ return 14 }}

    let denied = Command.new("{}") catch _ {{ return 15 }}
    if !fails_with(move denied, "std.process.permission_denied") {{ return 16 }}

    let invalid = Command.new("{}") catch _ {{ return 17 }}
    if !fails_with(move invalid, "std.process.invalid_input") {{ return 18 }}

    let relative = Command.new("./relative-helper") catch _ {{ return 19 }}
    let relative_status = relative.status() catch _ {{ return 20 }}
    if !exited_with(relative_status, 31) {{ return 21 }}

    var argument_command = Command.new("{}") catch _ {{ return 22 }}
    argument_command.arg("") catch _ {{ return 23 }}
    argument_command.arg("alpha beta") catch _ {{ return 24 }}
    var rejected_nul = false
    argument_command.arg("bad\0argument") catch failure {{
        if !failure.has_code("std.process.invalid_input") {{ return 25 }}
        rejected_nul = true
    }}
    if !rejected_nul {{ return 26 }}
    let argument_status = argument_command.status() catch _ {{ return 27 }}
    if !exited_with(argument_status, 0) {{ return 28 }}

    var oversized = String.with_capacity(2097152)
    var block: usize = 0
    while block < 32768 {{
        oversized.push_str("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        block += 1
    }}
    var oversized_command = Command.new("{}") catch _ {{ return 29 }}
    oversized_command.arg(&oversized as &str) catch _ {{ return 30 }}
    if !fails_with(move oversized_command, "std.process.invalid_input") {{ return 31 }}

    var attempt: usize = 0
    while attempt < 96 {{
        let repeated = Command.new("{}") catch _ {{ return 32 }}
        if !fails_with(move repeated, "std.process.not_found") {{ return 33 }}
        attempt += 1
    }}

    let final_success = Command.new("{}") catch _ {{ return 34 }}
    let final_status = final_success.status() catch _ {{ return 35 }}
    if !exited_with(final_status, 0) {{ return 36 }}
    return 0
}}
"#,
            success.display(),
            nonzero.display(),
            exit_127.display(),
            signaled.display(),
            missing.display(),
            denied.display(),
            invalid.display(),
            arguments.display(),
            success.display(),
            missing.display(),
            success.display(),
        ),
    );
    compile_and_execute_subprocess_lifecycle(&package_root.0, &standard_root);
}

fn compile_and_execute_subprocess_lifecycle(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_lifecycle_contract(image.image(), package_root);
}

fn compile_and_execute_subprocess_output(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_output_contract(image.image(), package_root);
}

fn compile_and_execute_configured_subprocess(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_configured_subprocess_contract(image.image(), package_root);
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
fn unicode_scalar_values_cross_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"noalloc func scalar_iteration_is_exact(): bool {
    let text: &str = "Aλ😀"
    if text.len() != 7 || text.char_count() != 3 { return false }
    var chars = text.chars()
    let first = chars.next() otherwise { return false }
    let second = chars.next() otherwise { return false }
    let third = chars.next() otherwise { return false }
    if first != 'A' || second != 'λ' || third != '\u{1F600}' { return false }
    let _extra = chars.next() otherwise { return true }
    return false
}

func main(): i32 {
    let face: char = '\u{1F600}'
    if face.code_point() != 128512 { return 1 }
    if face.utf8_len() != 4 || face.is_ascii() { return 2 }
    let digit = char.from_u32(57) otherwise { return 3 }
    if !digit.is_ascii_digit() || digit != '9' { return 4 }
    if !'\u{3000}'.is_whitespace() || '\u{3001}'.is_whitespace() { return 10 }
    if !'Ω'.is_alphabetic() || '3'.is_alphabetic() { return 11 }
    if !'ß'.is_lowercase() || !'İ'.is_uppercase() { return 12 }
    if !'٥'.is_decimal_digit() || '²'.is_decimal_digit() { return 13 }
    if !(digit < face) { return 5 }
    if !scalar_iteration_is_exact() { return 6 }
    var text = String.copy("A")
    text.push('λ')
    text.try_push('\u{1F600}') catch _ { return 7 }
    if (&text as &str) != "Aλ😀" { return 8 }
    let _surrogate = char.from_u32(55296) otherwise { return 0 }
    return 9
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
    execute_native_test(image.image(), &package_root.0, "unicode-scalars");
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

    let unicode: &str = "\xE3\x80\x80\xC2\xA0Nocter\xC2\x85\xE3\x80\x80"
    let unicode_start = unicode.trim_start()
    let unicode_end = unicode.trim_end()
    let unicode_both = unicode.trim()
    if unicode_start != "Nocter\xC2\x85\xE3\x80\x80" { return 11 }
    if unicode_end != "\xE3\x80\x80\xC2\xA0Nocter" { return 12 }
    if unicode_both != "Nocter" { return 13 }
    let unicode_addr = ptr.addr(unicode.ptr())
    if ptr.addr(unicode_start.ptr()) != unicode_addr + 5
        || ptr.addr(unicode_end.ptr()) != unicode_addr
        || ptr.addr(unicode_both.ptr()) != unicode_addr + 5 { return 15 }
    let zero_width_space: &str = "Nocter\xE2\x80\x8B"
    if zero_width_space.trim() != zero_width_space { return 14 }

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

blocking func open_and_drop(): void! {
    let stream = fs.read_dir(".")?
    return
}

blocking func open_fails_with(path: &str, code: &str): bool {
    let _stream = fs.read_dir(path) catch failure {
        return failure.has_code(code)
    }
    return false
}

blocking func inspect_directory(): i32! {
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

blocking func main(): i32 {
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

blocking func main(): i32! {
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
        r#"use std/io.{File, BlockingReader, BlockingWriter}
use std/io/buffer.{BlockingBufReader, BlockingBufWriter}
use std/string.String
use std/vec.Vec

struct InvalidBlockingReader {}

instance InvalidBlockingReader {
    impl BlockingReader

    blocking method &+self.read_blocking(buffer: &+[u8]): usize! {
        return buffer.len() + 1
    }
}

blocking func check_lines(): i32! {
    var reader = BlockingBufReader.with_capacity(File.open("lines.txt")?, 3)
    var line = String.with_capacity(64)
    let original_capacity = line.capacity()

    if !reader.read_line_into_blocking(&+line)? || (&line as &str) != "" { return 1 }
    if !reader.read_line_into_blocking(&+line)? || (&line as &str) != "alpha" { return 2 }
    if !reader.read_line_into_blocking(&+line)? || (&line as &str) != "lone\rbeta" { return 3 }
    if !reader.read_line_into_blocking(&+line)? || (&line as &str) != "😀 split" { return 4 }
    let final_line = reader.read_line_blocking()? otherwise { return 5 }
    if (&final_line as &str) != "final" { return 6 }
    if reader.read_line_into_blocking(&+line)? { return 7 }
    if (&line as &str) != "" { return 8 }
    if line.capacity() != original_capacity { return 9 }
    let _after_eof = reader.read_line_blocking()? otherwise { return 0 }
    return 10
}

blocking func check_invalid_utf8(): i32! {
    var reader = BlockingBufReader.with_capacity(File.open("invalid.txt")?, 2)
    var line = String.copy("sentinel")
    if !reader.read_line_into_blocking(&+line)? || (&line as &str) != "good" { return 1 }
    let _present = reader.read_line_into_blocking(&+line) catch failure {
        if !failure.has_code("std.string.invalid_utf8") { return 2 }
        if (&line as &str) != "" { return 3 }
        let _after_failure = reader.read_line_blocking()? otherwise { return 0 }
        return 4
    }
    return 5
}

blocking func check_zero_capacity_and_finish(): i32! {
    var reader = BlockingBufReader.with_capacity(File.open("single.txt")?, 0)
    let line = reader.read_line_blocking()? otherwise { return 1 }
    if (&line as &str) != "z" { return 2 }
    let _after_eof = reader.read_line_blocking()? otherwise {
        var source = reader.finish()
        source.close()
        return 0
    }
    return 3
}

blocking func check_closed_output(): i32! {
    var destination = File.create("writer.txt")?
    var nested = BlockingBufWriter.with_capacity(move destination, 8)
    var completed = BlockingBufWriter.with_capacity(move nested, 2)
    completed.write_text_blocking("abc")?
    nested = completed.finish()?
    var returned_destination = nested.finish()?
    returned_destination.close()

    var file = File.create("closed-file.txt")?
    file.close()
    file.write_text_blocking("not written") catch failure {
        if !failure.has_code("std.io.closed") { return 1 }
        var closed_destination = File.create("closed-buffer.txt")?
        closed_destination.close()
        var writer = BlockingBufWriter.with_capacity(move closed_destination, 1)
        writer.write_text_blocking("abc") catch first_failure {
            if !first_failure.has_code("std.io.closed") { return 2 }
            writer.write_text_blocking("not written") catch writer_failure {
                if !writer_failure.has_code("std.io.closed") { return 3 }
                return 0
            }
            return 4
        }
        return 5
    }
    return 6
}

blocking func check_invalid_count(): i32! {
    var reader = BlockingBufReader.with_capacity(InvalidBlockingReader {}, 4)
    var bytes: Vec<u8> = Vec [u8.truncate(0)]
    let _count = reader.read_blocking(&+bytes) catch failure {
        if !failure.has_code("std.io.invalid_read_count") { return 1 }
        if reader.read_blocking(&+bytes)? != 0 { return 2 }
        return 0
    }
    return 3
}

blocking func main(): i32 {
    let lines = check_lines() catch _ { return 20 }
    if lines != 0 { return lines }
    let invalid = check_invalid_utf8() catch _ { return 21 }
    if invalid != 0 { return 10 + invalid }
    let terminal = check_zero_capacity_and_finish() catch _ { return 22 }
    if terminal != 0 { return 30 + terminal }
    let output = check_closed_output() catch _ { return 23 }
    if output != 0 { return 40 + output }
    let invalid_count = check_invalid_count() catch _ { return 24 }
    if invalid_count != 0 { return 50 + invalid_count }
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
        r#"use std/io.BlockingReader
use std/io
use std/vec.Vec

func drop_stdin_wrapper(): void {
    let wrapper = io.stdin()
    return
}

blocking func main(): i32! {
    var input = io.stdin()
    let bytes = input.read_to_end_blocking()?
    if bytes.len() != 7 { return 1 }
    if bytes[0] != 97 || bytes[1] != 108 || bytes[2] != 112 { return 2 }
    if bytes[3] != 104 || bytes[4] != 97 || bytes[5] != 10 || bytes[6] != 255 { return 3 }

    input.close()
    var empty: Vec<u8> = Vec.empty()
    let _ = input.read_blocking(&+empty) catch closed_failure {
        if !closed_failure.has_code("std.io.closed") { return 4 }

        var after_close = io.stdin()
        if after_close.read_blocking(&+empty)? != 0 { return 5 }
        after_close.close()

        drop_stdin_wrapper()
        var after_drop = io.stdin()
        if after_drop.read_blocking(&+empty)? != 0 { return 6 }
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
use std/io/buffer.BlockingBufReader
use std/string.String

blocking func main(): i32! {
    var input = BlockingBufReader.with_capacity(io.stdin(), 3)
    var line = String.with_capacity(64)
    let original_capacity = line.capacity()

    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "" { return 1 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "alpha" { return 2 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "lone\rbeta" { return 3 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "😀 split" { return 4 }
    let final_line = input.read_line_blocking()? otherwise { return 5 }
    if (&final_line as &str) != "final" { return 6 }
    if input.read_line_into_blocking(&+line)? { return 7 }
    if (&line as &str) != "" { return 8 }
    if line.capacity() != original_capacity { return 9 }
    let _after_eof = input.read_line_blocking()? otherwise { return 42 }
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
    assert_eq!(cases.len(), 5);
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
fn standard_url_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"url\", module: \"./url\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["url"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard URL tests failed native compilation")
    };
    assert_eq!(cases.len(), 11);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_str_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"str\", module: \"./str\" }\n",
        "#test: { name: \"string\", module: \"./string\" }\n",
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
            ModuleIdentity::new(standard_package.clone(), ["str"]),
            ModuleIdentity::new(standard_package.clone(), ["string"]),
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
            panic!("standard text tests failed native compilation")
        };
        for case in cases {
            case_count += 1;
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 10);
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
    assert_eq!(cases.len(), 5);
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
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_network_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"net\", module: \"./net\" }\n",
        "#test: { name: \"net-stream-policy\", module: \"./internal/net\" }\n",
        "#test: { name: \"net-resolver-adapter\", module: \"./internal/net/darwin\" }\n",
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
            ModuleIdentity::new(standard_package.clone(), ["net"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net", "darwin"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 3);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("standard network tests failed native compilation")
        };
        case_count += cases.len();
        for case in cases {
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 30);
}

#[test]
fn standard_http_framing_contract_crosses_native_tests() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"http\", module: \"./http\" }\n");
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
            ModuleIdentity::new(standard_package.clone(), ["http"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard HTTP framing tests failed native compilation")
    };
    assert_eq!(cases.len(), 24);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_client_crosses_localhost_resolution_and_streaming_fixture() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &format!(
            "use std/http.{{Client, Request}}\n\
             use std/io.BlockingReader\n\
             use std/url.Url\n\
             \n\
             blocking func main(): i32 {{\n\
                 let url = Url.parse(\"http://localhost:{port}/from-fixture?q=1\") catch _ {{ return 1 }}\n\
                 let request = Request.get(move url) catch _ {{ return 2 }}\n\
                 let client = Client.new()\n\
                 var response = client.send_blocking(move request) catch _ {{ return 3 }}\n\
                 if response.status().code() != 200 {{ return 4 }}\n\
                 let _fixture = response.headers().first(\"x-fixture\") otherwise {{ return 5 }}\n\
                 let body = response.read_to_string_blocking() catch _ {{ return 6 }}\n\
                 if body != \"fixture\" {{ return 7 }}\n\
                 return 0\n\
             }}\n"
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
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        while !request.ends_with(b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before completing its head");
            request.extend_from_slice(&scratch[..received]);
        }
        assert_eq!(
            request,
            format!(
                "GET /from-fixture?q=1 HTTP/1.1\r\nhost: localhost:{port}\r\nconnection: close\r\ncontent-length: 0\r\n\r\n"
            )
            .into_bytes()
        );
        stream
            .write_all(
                b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 7\r\nX-Fixture: yes\r\n\r\nfi",
            )
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        stream.write_all(b"xture").unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "http-client", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn serve_plain_tls_peers_and_require_https_alpn(fixture: &std::net::TcpListener) {
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    for connection_index in 0..5 {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut record_header = [0_u8; 5];
        stream.read_exact(&mut record_header).unwrap();
        assert_eq!(
            record_header[0], 22,
            "client did not begin with a TLS handshake"
        );
        let record_len = usize::from(u16::from_be_bytes([record_header[3], record_header[4]]));
        let mut client_hello = vec![0_u8; record_len];
        stream.read_exact(&mut client_hello).unwrap();
        if connection_index == 1 || connection_index == 4 {
            assert!(
                client_hello
                    .windows(b"http/1.1".len())
                    .any(|window| window == b"http/1.1"),
                "HTTPS did not advertise the HTTP/1.1 ALPN protocol"
            );
        }
        if connection_index == 0 {
            stream.write_all(&[22, 3, 3, 0, 16, 1, 2]).unwrap();
        } else {
            stream.write_all(b"this is not a TLS record").unwrap();
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn plain_tls_rejection_source(port: u16, asynchronous: bool) -> String {
    let main = if asynchronous {
        "async func main(): i32 {\n\
             if !await rejects_async() { return 1 }\n\
             if !await rejects_https_async() { return 2 }\n\
             return 0\n\
         }"
    } else {
        "blocking func main(): i32 {\n\
             if !rejects_sync() { return 1 }\n\
             if !rejects_https_sync() { return 2 }\n\
             if !rejects_invalid_custom_anchor() { return 3 }\n\
             return 0\n\
         }"
    };
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/tls as tls\n\
         use std/tls.{{TlsStream, TrustAnchor}}\n\
         use std/url.Url\n\
         \n\
         blocking func rejects_sync(): bool {{\n\
             let _stream = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func rejects_https_sync(): bool {{\n\
             let client = Client.new()\n\
             let request = Request.get(Url.parse(\"https://localhost:{port}/\") catch _ {{\n\
                 return false\n\
             }}) catch _ {{ return false }}\n\
             let _response = client.send_with_timeout_blocking(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func rejects_async(): bool {{\n\
             let pending = tls.connect_with_timeout(\n\
                 \"localhost\",\n\
                 {port},\n\
                 Duration.from_seconds(1),\n\
             )\n\
             let _stream = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func rejects_https_async(): bool {{\n\
             let client = Client.new()\n\
             let request = Request.get(Url.parse(\"https://localhost:{port}/\") catch _ {{\n\
                 return false\n\
             }}) catch _ {{ return false }}\n\
             let pending_response = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             let _response = await pending_response catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func rejects_invalid_custom_anchor(): bool {{\n\
             let anchor = TrustAnchor.from_der(\"x\".bytes()) catch _ {{ return false }}\n\
             let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 &anchor,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         {main}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_tls_and_https_reject_plain_peers_and_https_advertises_http1() {
    use std::net::TcpListener;
    use std::thread;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let sync_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &plain_tls_rejection_source(port, false),
    );
    let async_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &plain_tls_rejection_source(port, true),
    );

    let server = thread::spawn(move || serve_plain_tls_peers_and_require_https_alpn(&fixture));
    execute_native_status(&sync_image, &package_root.0, "tls-plain-peer-sync", 0);
    execute_native_status(&async_image, &package_root.0, "tls-plain-peer-async", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn tls_handshake_timeout_source(port: u16, asynchronous: bool) -> String {
    let main = if asynchronous {
        "async func main(): i32 {\n\
             let timeout = Duration.from_milliseconds(500)\n\
             if !await async_tls_times_out(timeout) { return 1 }\n\
             let client = Client.new()\n\
             if !await async_https_times_out(&client, timeout) { return 2 }\n\
             return 0\n\
         }"
    } else {
        "blocking func main(): i32 {\n\
             let timeout = Duration.from_milliseconds(500)\n\
             if !sync_tls_times_out(timeout) { return 1 }\n\
             let client = Client.new()\n\
             if !sync_https_times_out(&client, timeout) { return 2 }\n\
             return 0\n\
         }"
    };
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/tls as tls\n\
         use std/tls.TlsStream\n\
         use std/url.Url\n\
         \n\
         blocking func sync_tls_times_out(timeout: Duration): bool {{\n\
             let _stream = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 timeout,\n\
             ) catch failure {{ return failure.has_code(\"std.net.timed_out\") }}\n\
             return false\n\
         }}\n\
         \n\
         async func async_tls_times_out(timeout: Duration): bool {{\n\
             let pending = tls.connect_with_timeout(\n\
                 \"localhost\",\n\
                 {port},\n\
                 timeout,\n\
             )\n\
             let _stream = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func sync_https_times_out(client: &Client, timeout: Duration): bool {{\n\
             let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let _response = client.send_with_timeout_blocking(move request, timeout) catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func async_https_times_out(client: &Client, timeout: Duration): bool {{\n\
             let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 timeout,\n\
             )\n\
             let _response = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         {main}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn tls_and_https_handshakes_share_the_fixed_timeout_contract() {
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let sync_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &tls_handshake_timeout_source(port, false),
    );
    let async_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &tls_handshake_timeout_source(port, true),
    );

    let server = thread::spawn(move || {
        fixture.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut peers = Vec::new();
        while peers.len() < 4 && Instant::now() < deadline {
            match fixture.accept() {
                Ok((stream, _)) => peers.push(stream),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("failed to accept a TLS timeout peer: {error}"),
            }
        }
        let accepted = peers.len();
        if accepted == 4 {
            thread::sleep(Duration::from_secs(1));
        }
        accepted
    });
    execute_native_status(
        &sync_image,
        &package_root.0,
        "tls-handshake-timeout-sync",
        0,
    );
    execute_native_status(
        &async_image,
        &package_root.0,
        "tls-handshake-timeout-async",
        0,
    );
    assert_eq!(
        server.join().unwrap(),
        4,
        "every TLS and HTTPS timeout path must reach the network peer"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
struct LocalTlsServer(std::process::Child);

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for LocalTlsServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn start_local_tls_server(
    port: u16,
    certificate: &Path,
    key: &Path,
    advertise_http1: bool,
) -> LocalTlsServer {
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let mut command = std::process::Command::new("/usr/bin/openssl");
    command.args([
        "s_server",
        "-accept",
        &port.to_string(),
        "-cert",
        certificate.to_str().unwrap(),
        "-key",
        key.to_str().unwrap(),
        "-quiet",
        "-www",
    ]);
    if advertise_http1 {
        command.args(["-alpn", "http/1.1"]);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = LocalTlsServer(child);
    for _ in 0..100 {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "local TLS server exited"
        );
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("local TLS server did not start");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
struct LocalTlsHttpServer {
    child: std::process::Child,
    exchange: Option<std::thread::JoinHandle<()>>,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl LocalTlsHttpServer {
    fn finish(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exchange.take().unwrap().join().unwrap();
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for LocalTlsHttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(exchange) = self.exchange.take() {
            let _ = exchange.join();
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn start_local_tls_http_server(
    port: u16,
    certificate: &Path,
    key: &Path,
    exchange_count: usize,
) -> LocalTlsHttpServer {
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let mut child = std::process::Command::new("/usr/bin/openssl")
        .args([
            "s_server",
            "-accept",
            &port.to_string(),
            "-cert",
            certificate.to_str().unwrap(),
            "-key",
            key.to_str().unwrap(),
            "-alpn",
            "http/1.1",
            "-quiet",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server_input = child.stdin.take().unwrap();
    let mut server_output = child.stdout.take().unwrap();
    let exchange = thread::spawn(move || {
        for _ in 0..exchange_count {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0_u8; 1];
                server_output.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            server_input
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            server_input.flush().unwrap();
        }
    });
    let mut server = LocalTlsHttpServer {
        child,
        exchange: Some(exchange),
    };
    for _ in 0..100 {
        assert!(
            server.child.try_wait().unwrap().is_none(),
            "local TLS HTTP server exited"
        );
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("local TLS HTTP server did not start");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn create_local_tls_fixture(configuration_root: &Path, output_root: &Path) {
    use std::process::Command;

    fn run(command: &mut Command) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "OpenSSL fixture generation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let root_key = output_root.join("root-key.pem");
    let root_certificate = output_root.join("root-cert.pem");
    let leaf_key = output_root.join("localhost-key.pem");
    let leaf_request = output_root.join("localhost.csr");
    let leaf_certificate = output_root.join("localhost-cert.pem");
    let root_der = output_root.join("root-cert.der");
    let root_configuration = configuration_root.join("root.cnf");
    let leaf_configuration = configuration_root.join("localhost.cnf");

    run(Command::new("/usr/bin/openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "365",
        ])
        .args(["-sha256", "-config"])
        .arg(&root_configuration)
        .arg("-keyout")
        .arg(&root_key)
        .arg("-out")
        .arg(&root_certificate));
    run(Command::new("/usr/bin/openssl")
        .args(["req", "-newkey", "rsa:2048", "-nodes", "-sha256", "-config"])
        .arg(&leaf_configuration)
        .arg("-keyout")
        .arg(&leaf_key)
        .arg("-out")
        .arg(&leaf_request));
    run(Command::new("/usr/bin/openssl")
        .args(["x509", "-req", "-in"])
        .arg(&leaf_request)
        .arg("-CA")
        .arg(&root_certificate)
        .arg("-CAkey")
        .arg(&root_key)
        .args([
            "-CAcreateserial",
            "-set_serial",
            "1000",
            "-days",
            "300",
            "-sha256",
        ])
        .arg("-extfile")
        .arg(&leaf_configuration)
        .args(["-extensions", "certificate", "-out"])
        .arg(&leaf_certificate));
    run(Command::new("/usr/bin/openssl")
        .args(["x509", "-in"])
        .arg(&root_certificate)
        .args(["-outform", "der", "-out"])
        .arg(&root_der));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn create_expired_local_tls_certificate(configuration_root: &Path, output_root: &Path) {
    use std::process::Command;

    std::fs::write(output_root.join("index.txt"), []).unwrap();
    std::fs::write(output_root.join("serial"), b"1001\n").unwrap();
    let output = Command::new("/usr/bin/openssl")
        .current_dir(output_root)
        .args(["ca", "-batch", "-config"])
        .arg(configuration_root.join("expired.cnf"))
        .args([
            "-startdate",
            "20000101000000Z",
            "-enddate",
            "20000102000000Z",
            "-in",
            "localhost.csr",
            "-out",
            "expired-localhost-cert.pem",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expired OpenSSL fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_augments_system_roots_and_preserves_hostname_authentication() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, true);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = compiler_root.join("../std");
    let sync_main = format!(
        "blocking func main(): i32 {{\n\
             let _system = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\", {port}, Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 if !failure.has_code(\"std.net.tls_failed\") {{ return 1 }}\n\
                 let certificate = fs.read(\"root-cert.der\") catch _ {{ return 2 }}\n\
                 let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 3 }}\n\
                 var stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"localhost\", {port}, &anchor, Duration.from_seconds(1),\n\
                 ) catch _ {{ return 4 }}\n\
                 stream.close()\n\
                 if !rejects_mismatched_name(&anchor) {{ return 5 }}\n\
                 return 0\n\
             }}\n\
             return 6\n\
         }}"
    );
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             if !await accepts_asynchronously(&anchor) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/time.Duration\n\
             use std/tls as tls\n\
             use std/tls.{{TlsStream, TrustAnchor}}\n\
             use std/vec.Vec\n\
             \n\
             blocking func rejects_mismatched_name(anchor: &TrustAnchor): bool {{\n\
                 let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"127.0.0.1\", {port}, anchor, Duration.from_seconds(1),\n\
                 ) catch failure {{ return failure.has_code(\"std.net.tls_failed\") }}\n\
                 return false\n\
             }}\n\
             \n\
             async func accepts_asynchronously(anchor: &TrustAnchor): bool {{\n\
                 let pending = tls.connect_with_trust_anchor_and_timeout(\n\
                     \"localhost\", {port}, anchor, Duration.from_seconds(1),\n\
                 )\n\
                 var stream = await pending catch _ {{ return false }}\n\
                 stream.close()\n\
                 return true\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "tls-custom-trust-sync", 0);
    execute_native_status(&async_image, &package_root.0, "tls-custom-trust-async", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_does_not_override_certificate_validity() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    create_expired_local_tls_certificate(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("expired-localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, true);

    let standard_root = compiler_root.join("../std");
    package_root.source(
        "main.nct",
        &format!(
            "use std/fs\n\
             use std/time.Duration\n\
             use std/tls.{{TlsStream, TrustAnchor}}\n\
             \n\
             blocking func main(): i32 {{\n\
                 let certificate = fs.read(\"root-cert.der\") catch _ {{ return 1 }}\n\
                 let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
                 let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"localhost\", {port}, &anchor, Duration.from_seconds(1),\n\
                 ) catch failure {{\n\
                     if failure.has_code(\"std.net.tls_failed\") {{ return 0 }}\n\
                     return 3\n\
                 }}\n\
                 return 4\n\
             }}\n"
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
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();
    execute_native_status(image.image(), &package_root.0, "tls-expired-certificate", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_crosses_sync_and_async_https_without_a_second_http_codec() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let server = start_local_tls_http_server(port, &certificate, &key, 2);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = compiler_root.join("../std");
    let sync_main = "blocking func main(): i32 {\n\
             let certificate = fs.read(\"root-cert.der\") catch _ { return 1 }\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ { return 2 }\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             return accepts_sync(&client)\n\
         }";
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !await accepts_async(&client) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/http.{{Client, Request}}\n\
             use std/time.Duration\n\
             use std/tls.TrustAnchor\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             blocking func accepts_sync(client: &Client): i32 {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return 1 }}\n\
                 let request = Request.get(move url) catch _ {{ return 2 }}\n\
                 var response = client.send_with_timeout_blocking(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 ) catch failure {{\n\
                     if failure.has_code(\"std.net.tls_failed\") {{ return 3 }}\n\
                     if failure.has_code(\"std.net.timed_out\") {{ return 4 }}\n\
                     if failure.has_code(\"std.http.premature_eof\") {{ return 5 }}\n\
                     if failure.has_code(\"std.http.invalid_syntax\") {{ return 6 }}\n\
                     return 7\n\
                 }}\n\
                 let accepted = response.status().code() == 200\n\
                 response.close()\n\
                 if accepted {{ return 0 }}\n\
                 return 4\n\
             }}\n\
             \n\
             async func accepts_async(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 var response = await pending catch _ {{ return false }}\n\
                 let accepted = response.status().code() == 200\n\
                 response.close()\n\
                 return accepted\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "https-custom-trust-sync", 0);
    execute_native_status(&async_image, &package_root.0, "https-custom-trust-async", 0);
    server.finish();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn https_requires_the_negotiated_http1_application_protocol() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, false);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = compiler_root.join("../std");
    let sync_main = "blocking func main(): i32 {\n\
             let certificate = fs.read(\"root-cert.der\") catch _ { return 1 }\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ { return 2 }\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !rejects_sync(&client) { return 3 }\n\
             return 0\n\
         }";
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !await rejects_async(&client) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/http.{{Client, Request}}\n\
             use std/time.Duration\n\
             use std/tls.TrustAnchor\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             blocking func rejects_sync(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 var response = client.send_with_timeout_blocking(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 ) catch failure {{ return failure.has_code(\"std.net.tls_failed\") }}\n\
                 response.close()\n\
                 return false\n\
             }}\n\
             \n\
             async func rejects_async(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 var response = await pending catch failure {{\n\
                     return failure.has_code(\"std.net.tls_failed\")\n\
                 }}\n\
                 response.close()\n\
                 return false\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "https-alpn-required-sync", 0);
    execute_native_status(
        &async_image,
        &package_root.0,
        "https-alpn-required-async",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_client_crosses_reactor_and_fragmented_body_fixture() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &format!(
            "use std/http.{{Client, Request}}\n\
             use std/url.Url\n\
             \n\
             async func main(): i32 {{\n\
                 let url = Url.parse(\"http://localhost:{port}/async?q=1\") catch _ {{ return 1 }}\n\
                 var request = Request.post(move url) catch _ {{ return 2 }}\n\
                 request.append_header_text(\"X-Request\", \"phase3\") catch _ {{ return 3 }}\n\
                 request.set_text_body(\"payload\")\n\
                 let client = Client.new()\n\
                 let pending = client.send(move request)\n\
                 var response = await pending catch _ {{ return 5 }}\n\
                 if response.status().code() != 200 {{ return 6 }}\n\
                 let _fixture = response.headers().first(\"x-fixture\") otherwise {{ return 7 }}\n\
                 let text = await response.read_to_string() catch _ {{ return 8 }}\n\
                 if text != \"fragmented\" {{ return 9 }}\n\
                 return 0\n\
             }}\n"
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
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        let head_end = loop {
            if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                break offset + 4;
            }
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(
                received, 0,
                "async HTTP client closed before completing its head"
            );
            request.extend_from_slice(&scratch[..received]);
        };
        while request.len() < head_end + 7 {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(
                received, 0,
                "async HTTP client closed before completing its body"
            );
            request.extend_from_slice(&scratch[..received]);
        }
        assert_eq!(
            &request[..head_end],
            format!(
                "POST /async?q=1 HTTP/1.1\r\nx-request: phase3\r\nhost: localhost:{port}\r\nconnection: close\r\ncontent-length: 7\r\n\r\n"
            )
            .as_bytes()
        );
        assert_eq!(&request[head_end..], b"payload");
        stream
            .write_all(
                b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Fixture: yes\r\n\r\n4\r\nfrag\r\n",
            )
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        stream.write_all(b"6\r\nmented\r\n0\r\n\r\n").unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "async-http-client", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn async_http_timeout_source(abandoned_port: u16, fixture_port: u16) -> String {
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/url.Url\n\
         use std/vec.Vec\n\
         \n\
         async func head_times_out(client: &Client, url: Url, timeout: Duration): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 timeout,\n\
             )\n\
             let _response = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func body_times_out(client: &Client, url: Url, timeout: Duration): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             var response = await pending catch _ {{ return false }}\n\
             var buffer: Vec<u8> = Vec [u8.truncate(0)]\n\
             let abandoned = response.read(&+buffer)\n\
             drop abandoned\n\
             let _body = await response.read_to_end_with_timeout(timeout) catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func truncated_peer_fails(client: &Client, url: Url): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             var response = await pending catch _ {{ return false }}\n\
             var buffer: Vec<u8> = Vec [\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
             ]\n\
             let first = await response.read_with_timeout(\n\
                 &+buffer,\n\
                 Duration.from_seconds(1),\n\
             ) catch _ {{ return false }}\n\
             if first != 2 {{ return false }}\n\
             let _second = await response.read_with_timeout(\n\
                 &+buffer,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.http.premature_eof\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func main(): i32 {{\n\
             let client = Client.new()\n\
             let abandoned_request = Request.get(\n\
                 Url.parse(\"http://localhost:{abandoned_port}/abandoned\") catch _ {{ return 1 }},\n\
             ) catch _ {{ return 2 }}\n\
             let abandoned = client.send(move abandoned_request)\n\
             drop abandoned\n\
             let short = Duration.from_seconds(1)\n\
             let head_url = Url.parse(\"http://localhost:{fixture_port}/head\") catch _ {{ return 4 }}\n\
             if !await head_times_out(&client, move head_url, short) {{ return 5 }}\n\
             let body_url = Url.parse(\"http://localhost:{fixture_port}/body\") catch _ {{ return 6 }}\n\
             if !await body_times_out(&client, move body_url, short) {{ return 7 }}\n\
             let truncated_url = Url.parse(\"http://localhost:{fixture_port}/truncated\") catch _ {{ return 8 }}\n\
             if !await truncated_peer_fails(&client, move truncated_url) {{ return 9 }}\n\
             return 0\n\
         }}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_timeouts_and_abandoned_operations_preserve_ownership() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    fn read_request_head(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        while request.windows(4).all(|bytes| bytes != b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before sending its head");
            request.extend_from_slice(&scratch[..received]);
        }
        request
    }

    let abandoned_fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let abandoned_port = abandoned_fixture.local_addr().unwrap().port();
    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let fixture_port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &async_http_timeout_source(abandoned_port, fixture_port),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    let server = thread::spawn(move || {
        let (mut head_stream, _) = fixture.accept().unwrap();
        let head_request = read_request_head(&mut head_stream);
        assert!(head_request.starts_with(b"GET /head HTTP/1.1\r\n"));
        thread::sleep(Duration::from_millis(1200));
        drop(head_stream);

        let (mut body_stream, _) = fixture.accept().unwrap();
        let body_request = read_request_head(&mut body_stream);
        assert!(body_request.starts_with(b"GET /body HTTP/1.1\r\n"));
        body_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")
            .unwrap();
        thread::sleep(Duration::from_millis(1200));
        let _ = body_stream.write_all(b"late");

        let (mut truncated_stream, _) = fixture.accept().unwrap();
        let truncated_request = read_request_head(&mut truncated_stream);
        assert!(truncated_request.starts_with(b"GET /truncated HTTP/1.1\r\n"));
        truncated_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "async-http-timeouts", 0);
    server.join().unwrap();
    drop(abandoned_fixture);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_request_body_observes_write_backpressure_timeout() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let body_chunk = "x".repeat(1024);
    package_root.source(
        "main.nct",
        &format!(
            "use std/http.{{Client, Method, Request}}\n\
             use std/time.Duration\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             async func main(): i32 {{\n\
                 let body_text = \"{body_chunk}\".repeat(2048)\n\
                 let body_view: &str = &body_text\n\
                 let body = Vec.from_slice(body_view.bytes())\n\
                 let url = Url.parse(\"http://localhost:{port}/backpressure\") catch _ {{ return 1 }}\n\
                 var request = Request.new(Method.post(), move url) catch _ {{ return 2 }}\n\
                 request.set_body(move body)\n\
                 let client = Client.new()\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 let _response = await pending catch failure {{\n\
                     if failure.has_code(\"std.net.timed_out\")\n\
                         && failure.message() == \"while writing the HTTP request body\" {{\n\
                         return 0\n\
                     }}\n\
                     return 4\n\
                 }}\n\
                 return 5\n\
             }}\n"
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
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request_prefix = Vec::new();
        let mut scratch = [0_u8; 256];
        while request_prefix.windows(4).all(|bytes| bytes != b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before its request body");
            request_prefix.extend_from_slice(&scratch[..received]);
        }
        assert!(request_prefix.starts_with(b"POST /backpressure HTTP/1.1\r\n"));
        thread::sleep(Duration::from_millis(1200));
    });

    execute_native_status(
        image.image(),
        &package_root.0,
        "async-http-write-timeout",
        0,
    );
    server.join().unwrap();
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
    assert_eq!(cases.len(), 16);
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
    assert_eq!(cases.len(), 23);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_async_sleep_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        include_str!("../../../tests/fixtures/native/async_io_defaults.nct"),
    );
    execute_native_status(&image, &package_root.0, "async-io-defaults", 0);
}

#[test]
fn structured_async_join_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
fn suspended_child_can_read_parent_storage_without_parent_side_liveness() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
fn public_async_tcp_crosses_the_complete_native_session() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/io.{Reader, Writer}\n\
         use std/io/buffer.{BufReader, BufWriter}\n\
         use std/net\n\
         use std/string.String\n\
         use std/task\n\
         \n\
         struct Sent {}\n\
         \n\
         async func send(stream: net.TcpStream): Sent! {\n\
             var writer = BufWriter.with_capacity(move stream, 0)\n\
             await writer.write_text(\"\\nalpha\\r\\n😀 split\\nfinal\")?\n\
             var returned = await writer.finish()?\n\
             returned.close()\n\
             return Sent {}\n\
         }\n\
         \n\
         async func receive(stream: net.TcpStream): i32! {\n\
             var reader = BufReader.with_capacity(move stream, 0)\n\
             var line = String.with_capacity(16)\n\
             if !await reader.read_line_into(&+line)? || (&line as &str) != \"\" { return 1 }\n\
             if !await reader.read_line_into(&+line)? || (&line as &str) != \"alpha\" { return 2 }\n\
             if !await reader.read_line_into(&+line)? || (&line as &str) != \"😀 split\" { return 3 }\n\
             let final_line = await reader.read_line()? otherwise { return 4 }\n\
             if (&final_line as &str) != \"final\" { return 5 }\n\
             let _after_eof = await reader.read_line()? otherwise {\n\
                 var source = reader.finish()\n\
                 source.close()\n\
                 return 0\n\
             }\n\
             return 6\n\
         }\n\
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
             let client = move client_result?\n\
             let accepted = move accepted_result?\n\
             let streams = await task.join(\n\
                 send(move client),\n\
                 receive(move accepted.0),\n\
             )\n\
             let status = move streams.1?\n\
             if status != 0 { return status }\n\
             let _sent = move streams.0?\n\
             return status\n\
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
    execute_native_status(image.image(), &package_root.0, "async-buffers", 0);
}

#[test]
fn standard_async_buffer_cancellation_preserves_reader_prefix_and_terminates_writer() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/io.{Reader, Writer}\n\
         use std/io/buffer.{BufReader, BufWriter}\n\
         use std/string.String\n\
         use std/task\n\
         use std/task.Timeout\n\
         use std/time\n\
         use std/time.Duration\n\
         use std/vec.Vec\n\
         \n\
         struct Counter {\n\
             value: i32\n\
         }\n\
         \n\
         struct StagedReader {\n\
             stage: usize\n\
             counter: &+Counter\n\
         }\n\
         \n\
         instance StagedReader {\n\
             impl Reader\n\
         \n\
             async method &+self.read(buffer: &+[u8]): usize! {\n\
                 if self.stage == 0 {\n\
                     buffer[0] = 112\n\
                     buffer[1] = 97\n\
                     buffer[2] = 114\n\
                     buffer[3] = 116\n\
                     buffer[4] = 105\n\
                     buffer[5] = 97\n\
                     buffer[6] = 108\n\
                     self.stage = 1\n\
                     return 7\n\
                 }\n\
                 if self.stage == 1 {\n\
                     await time.sleep(Duration.from_milliseconds(20))\n\
                     buffer[0] = 32\n\
                     buffer[1] = 108\n\
                     buffer[2] = 105\n\
                     buffer[3] = 110\n\
                     buffer[4] = 101\n\
                     buffer[5] = 10\n\
                     self.stage = 2\n\
                     return 6\n\
                 }\n\
                 return 0\n\
             }\n\
         }\n\
         \n\
         drop StagedReader(&+self) {\n\
             self.counter.value += 1\n\
             return\n\
         }\n\
         \n\
         struct DelayedWriter {\n\
             counter: &+Counter\n\
         }\n\
         \n\
         instance DelayedWriter {\n\
             impl Writer\n\
         \n\
             async method &+self.write(bytes: &[u8]): void! {\n\
                 await time.sleep(Duration.from_milliseconds(20))\n\
                 let _length = bytes.len()\n\
                 return\n\
             }\n\
         }\n\
         \n\
         drop DelayedWriter(&+self) {\n\
             self.counter.value += 1\n\
             return\n\
         }\n\
         \n\
         struct InvalidReader {}\n\
         \n\
         instance InvalidReader {\n\
             impl Reader\n\
         \n\
             async method &+self.read(buffer: &+[u8]): usize! {\n\
                 return buffer.len() + 1\n\
             }\n\
         }\n\
         \n\
         struct FailingWriter {}\n\
         \n\
         instance FailingWriter {\n\
             impl Writer\n\
         \n\
             async method &+self.write(bytes: &[u8]): void! {\n\
                 let _length = bytes.len()\n\
                 return error.new(\"fixture.write\", \"fixture rejected output\")\n\
             }\n\
         }\n\
         \n\
         struct Flushed {}\n\
         \n\
         async func flush_marker(writer: &+BufWriter<DelayedWriter>): Flushed! {\n\
             await writer.flush()?\n\
             return Flushed {}\n\
         }\n\
         \n\
         async func check_reader(counter: &+Counter): i32! {\n\
             var reader = BufReader.with_capacity(\n\
                 StagedReader { stage: 0, counter: counter },\n\
                 8,\n\
             )\n\
             let cancelled = await task.with_timeout(\n\
                 reader.read_line(),\n\
                 Duration.from_milliseconds(2),\n\
             )\n\
             match move cancelled {\n\
                 Timeout.completed(_) { return 1 }\n\
                 Timeout.elapsed {}\n\
             }\n\
             var prefix: Vec<u8> = Vec [\n\
                 u8.truncate(0), u8.truncate(0), u8.truncate(0), u8.truncate(0),\n\
                 u8.truncate(0), u8.truncate(0), u8.truncate(0),\n\
             ]\n\
             if await reader.read(&+prefix)? != 7 { return 2 }\n\
             let prefix_text = String.from_utf8(&prefix)?\n\
             if (&prefix_text as &str) != \"partial\" { return 3 }\n\
             let remainder = await reader.read_line()? otherwise { return 4 }\n\
             if (&remainder as &str) != \" line\" { return 5 }\n\
             let _after_eof = await reader.read_line()? otherwise { return 0 }\n\
             return 6\n\
         }\n\
         \n\
         async func check_writer(counter: &+Counter): i32! {\n\
             var writer = BufWriter.with_capacity(DelayedWriter { counter: counter }, 8)\n\
             await writer.write_text(\"x\")?\n\
             let cancelled = await task.with_timeout(\n\
                 flush_marker(&+writer),\n\
                 Duration.from_milliseconds(2),\n\
             )\n\
             match move cancelled {\n\
                 Timeout.completed(_) { return 1 }\n\
                 Timeout.elapsed {}\n\
             }\n\
             await writer.write_text(\"y\") catch failure {\n\
                 if failure.has_code(\"std.io.closed\") { return 0 }\n\
                 return 2\n\
             }\n\
             return 3\n\
         }\n\
         \n\
         async func check_invalid_count(): i32! {\n\
             var reader = BufReader.with_capacity(InvalidReader {}, 4)\n\
             var bytes: Vec<u8> = Vec [u8.truncate(0)]\n\
             let _count = await reader.read(&+bytes) catch failure {\n\
                 if !failure.has_code(\"std.io.invalid_read_count\") { return 1 }\n\
                 if await reader.read(&+bytes)? != 0 { return 2 }\n\
                 return 0\n\
             }\n\
             return 3\n\
         }\n\
         \n\
         async func check_failed_writer(): i32! {\n\
             var writer = BufWriter.with_capacity(FailingWriter {}, 1)\n\
             await writer.write_text(\"ab\") catch failure {\n\
                 if !failure.has_code(\"fixture.write\") { return 1 }\n\
                 await writer.flush() catch terminal {\n\
                     if terminal.has_code(\"std.io.closed\") { return 0 }\n\
                     return 2\n\
                 }\n\
                 return 3\n\
             }\n\
             return 4\n\
         }\n\
         \n\
         async func main(): i32! {\n\
             var reader_drops = Counter { value: 0 }\n\
             let reader = await check_reader(&+reader_drops)?\n\
             if reader != 0 { return reader }\n\
             if reader_drops.value != 1 { return 7 }\n\
             var writer_drops = Counter { value: 0 }\n\
             let writer = await check_writer(&+writer_drops)?\n\
             if writer != 0 { return 10 + writer }\n\
             if writer_drops.value != 1 { return 14 }\n\
             let invalid = await check_invalid_count()?\n\
             if invalid != 0 { return 20 + invalid }\n\
             let failed = await check_failed_writer()?\n\
             if failed != 0 { return 30 + failed }\n\
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
        "async-buffer-cancellation",
        0,
    );
}

#[test]
fn public_async_udp_crosses_readiness_timeout_cancellation_and_datagram_boundaries() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../tests/fixtures/native/async_udp.nct"),
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = fs::canonicalize(compiler_root.join("../std")).unwrap();
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
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

const RECOVERABLE_JSON_FLOAT_TEST_SOURCE: &str = concat!(
    "use /json.Number\n",
    "use /mem\n",
    "test recoverable_json_float_propagates_allocator_failure {\n",
    "    var allocator = mem.failing_try_allocator_for_test()\n",
    "    let _number = Number.try_from_f64(&+allocator, 0.1) catch failure {\n",
    "        if failure.has_code(\"std.mem.invalid_argument\") { return }\n",
    "        return error.new(\"std.json.allocator\", \"wrong float allocator failure\")\n",
    "    }\n",
    "    return error.new(\"std.json.allocator\", \"invalid allocator created a number\")\n",
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
            RECOVERABLE_JSON_FLOAT_TEST_SOURCE.to_string(),
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
    assert_eq!(case_count, 30);
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
    assert_eq!(case_count, 23);
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
        nocter_source::SourceIdentityDomain::new(),
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
fn execute_native_status(image: &NativeImage, root: &Path, name: &str, expected: i32) {
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
        Some(expected),
        "native image exited with {status:?}"
    );
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_native_status(_image: &NativeImage, _root: &Path, _name: &str, _expected: i32) {}

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
fn execute_subprocess_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-contract");
    let helper = root.join("subprocess-helper");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        b"#!/bin/sh\n[ \"$#\" -eq 2 ] || exit 21\n[ \"$1\" = \"alpha beta\" ] || exit 22\n[ \"$2\" = \"\" ] || exit 23\n[ \"$NOCTER_SUBPROCESS_TEST\" = \"inherited\" ] || exit 24\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .env("NOCTER_SUBPROCESS_TEST", "inherited")
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "subprocess contract executable exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_output_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-output-contract");
    let helper = root.join("capture-helper");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        concat!(
            "#!/bin/sh\n",
            "i=0\n",
            "while [ \"$i\" -lt 4096 ]; do\n",
            "  printf 'OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO'\n",
            "  printf 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE' >&2\n",
            "  i=$((i + 1))\n",
            "done\n",
            "printf '\\000\\377'\n",
            "printf '\\000\\376' >&2\n",
            "exit 23\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        ("empty-capture-helper", "#!/bin/sh\nexit 0\n"),
        (
            "text-capture-helper",
            "#!/bin/sh\nprintf 'hello\\n'\nprintf 'warning\\n' >&2\nexit 0\n",
        ),
        (
            "signal-capture-helper",
            "#!/bin/sh\nprintf 'signal-out'\nprintf 'signal-error' >&2\nkill -TERM $$\nexit 90\n",
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    for (name, shell) in [
        ("ordinary", None),
        ("closed-stdout", Some("exec 1>&-; exec \"$1\"")),
        ("closed-stderr", Some("exec 2>&-; exec \"$1\"")),
    ] {
        let status = match shell {
            Some(script) => Command::new("/bin/sh")
                .current_dir(root)
                .arg("-c")
                .arg(script)
                .arg("nocter-capture-test")
                .arg(&executable)
                .status()
                .unwrap(),
            None => Command::new(&executable)
                .current_dir(root)
                .status()
                .unwrap(),
        };
        assert_eq!(
            status.code(),
            Some(0),
            "subprocess output contract {name} exited with {status:?}"
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_configured_subprocess_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("configured-subprocess-contract");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    let workspace = root.join("configured-workspace");
    fs::create_dir(&workspace).unwrap();
    let environment_helper = workspace.join("environment-helper");
    fs::write(
        &environment_helper,
        concat!(
            "#!/bin/sh\n",
            "[ -f ./working-directory-marker ] || exit 31\n",
            "[ \"$KEEP\" = \"final=value\" ] || exit 32\n",
            "[ \"${REMOVE+x}\" = \"\" ] || exit 33\n",
            "[ \"${NOCTER_INHERITED+x}\" = \"\" ] || exit 34\n",
            "exit 0\n",
        ),
    )
    .unwrap();
    fs::write(workspace.join("working-directory-marker"), "ready\n").unwrap();
    fs::set_permissions(&environment_helper, fs::Permissions::from_mode(0o755)).unwrap();

    let inherited_helper = root.join("inherited-helper");
    fs::write(
        &inherited_helper,
        concat!(
            "#!/bin/sh\n",
            "[ \"$NOCTER_INHERITED\" = \"parent\" ] || exit 41\n",
            "[ \"$NOCTER_CHANGED\" = \"child=value\" ] || exit 42\n",
            "[ \"${NOCTER_REMOVED+x}\" = \"\" ] || exit 43\n",
            "exit 0\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&inherited_helper, fs::Permissions::from_mode(0o755)).unwrap();

    let transfer_helper = root.join("transfer-helper");
    fs::write(
        &transfer_helper,
        concat!(
            "#!/bin/sh\n",
            "i=0\n",
            "while [ \"$i\" -lt 2048 ]; do\n",
            "  printf 'OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO'\n",
            "  printf 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE' >&2\n",
            "  i=$((i + 1))\n",
            "done\n",
            "/bin/cat\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&transfer_helper, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        (
            "empty-input-helper",
            "#!/bin/sh\nif IFS= read -r line; then exit 51; fi\nexit 0\n",
        ),
        ("early-close-helper", "#!/bin/sh\nexit 0\n"),
    ] {
        let helper = root.join(name);
        fs::write(&helper, source).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let status = Command::new(&executable)
        .current_dir(root)
        .env("NOCTER_INHERITED", "parent")
        .env("NOCTER_CHANGED", "parent")
        .env("NOCTER_REMOVED", "parent")
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "configured subprocess contract exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_lifecycle_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-lifecycle-contract");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        ("success-helper", "#!/bin/sh\nexit 0\n"),
        ("nonzero-helper", "#!/bin/sh\nexit 23\n"),
        ("exit-127-helper", "#!/bin/sh\nexit 127\n"),
        ("signal-helper", "#!/bin/sh\nkill -TERM $$\nexit 90\n"),
        ("relative-helper", "#!/bin/sh\nexit 31\n"),
        (
            "argument-helper",
            "#!/bin/sh\n[ \"$#\" -eq 2 ] || exit 40\n[ \"$1\" = \"\" ] || exit 41\n[ \"$2\" = \"alpha beta\" ] || exit 42\nexit 0\n",
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let denied = root.join("denied-helper");
    fs::write(&denied, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o644)).unwrap();

    let invalid = root.join("invalid-helper");
    fs::write(&invalid, "this is not an executable image\n").unwrap();
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "subprocess lifecycle contract exited with {status:?}"
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
fn execute_subprocess_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_output_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_configured_subprocess_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_lifecycle_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn prepare_path_directory_fixture(_root: &Path) {}
