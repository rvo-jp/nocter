use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_machine::MachineProgram;
use nocter_mir::lower_executable;
use nocter_model::CompilationTarget;
use nocter_target_program::{
    ExecutableProgram, PrimitiveBinding, PrimitiveRegistry, PrimitiveRole, TargetProgram,
    ToolchainSnapshot,
};
use nocter_test_support::CompilerFixture;

#[test]
fn constant_process_crosses_the_complete_native_pipeline() {
    let machine = lower_machine("func main(): i32 { 42 }\n");
    let first = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let second = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let first_image = nocter_macho::MachOImage::build(&first).unwrap();
    let second_image = nocter_macho::MachOImage::build(&second).unwrap();

    assert_eq!(first_image, second_image);
    assert!(!first_image.bytes().is_empty());
    execute_and_assert_status(&first_image, 42);
}

#[test]
fn scalar_call_and_arithmetic_cross_the_complete_native_pipeline() {
    let machine = lower_machine(
        "func double(value: i32): i32 { value * 2 }\n\
         func main(): i32 { double(20) + 2 }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn scalar_comparison_and_branch_cross_the_complete_native_pipeline() {
    let machine = lower_machine(
        "func main(): i32 {\n\
             if !(3 < 4) { return 1 }\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn narrow_signed_values_cross_calls_and_borrowed_comparisons() {
    let machine = lower_machine(
        "func ordered(left: i8, right: i8): bool { left < right }\n\
         func main(): i32 {\n\
             if !ordered(-2, 1) { return 1 }\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn guarded_signed_arithmetic_crosses_the_complete_native_pipeline() {
    let machine = lower_machine("func main(): i32 { ((-21 / -3) << 3) - (7 % 4) - 11 }\n");
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn block_parameters_cross_control_flow_edges_natively() {
    let machine = lower_machine(
        "func choose(condition: bool): i32 {\n\
             if condition { 1 } else { 2 }\n\
         }\n\
         func main(): i32 { choose(true) * 40 + choose(false) }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn memory_values_cross_control_flow_edges_natively() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func choose(condition: bool): Large {\n\
             if condition {\n\
                 Large { first: 20, second: 20, third: 42 }\n\
             } else {\n\
                 Large { first: 1, second: 1, third: 1 }\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let fallback = choose(false)\n\
             if fallback.third == 1 {\n\
                 let selected = choose(true)\n\
                 return selected.third\n\
             }\n\
             return 2\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn optional_tag_switches_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func force(value: i32?): i32 { value! }\n\
         func main(): i32 { force(42) }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn static_text_and_two_word_view_transport_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func relay(text: &str): &str { text }\n\
         func main(): i32 {\n\
             let text = relay(\"hello\")\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn two_word_views_cross_the_register_window_and_outgoing_stack() {
    let machine = lower_machine(
        "func last(\n\
             a: &str, b: &str, c: &str, d: &str, e: &str,\n\
             f: &str, g: &str, h: &str, i: &str,\n\
         ): &str { i }\n\
         func main(): i32 {\n\
             let text = last(\"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\", \"i\")\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn layout_owned_aggregate_bytes_cross_the_native_pipeline() {
    let machine = lower_machine(
        "copy struct Triple { first: u8\n    second: u8\n    third: u8 }\n\
         copy struct Direct { first: i32\n    second: i32\n    third: i32 }\n\
         copy struct Large { first: i64\n    second: i64\n    third: i64 }\n\
         func main(): i32 {\n\
             let triple = Triple { first: 1, second: 2, third: 3 }\n\
             let direct = Direct { first: 20, second: 20, third: 2 }\n\
             let large = Large { first: 1, second: 2, third: 42 }\n\
             if triple.third == 3 {\n\
                 if large.third == 42 {\n\
                     return direct.first + direct.second + direct.third\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn indirect_aggregate_arguments_and_results_cross_the_native_pipeline() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func identity(value: Large): Large { value }\n\
         func relay(value: Large): Large { identity(value) }\n\
         func main(): i32 {\n\
             let value = relay(Large { first: 1, second: 2, third: 42 })\n\
             return value.third\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn indirect_aggregate_arguments_cross_the_outgoing_stack_boundary() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func ninth(\n\
             a: Large, b: Large, c: Large, d: Large, e: Large,\n\
             f: Large, g: Large, h: Large, i: Large,\n\
         ): Large { i }\n\
         func main(): i32 {\n\
             let value = ninth(\n\
                 Large { first: 0, second: 0, third: 1 },\n\
                 Large { first: 0, second: 0, third: 2 },\n\
                 Large { first: 0, second: 0, third: 3 },\n\
                 Large { first: 0, second: 0, third: 4 },\n\
                 Large { first: 0, second: 0, third: 5 },\n\
                 Large { first: 0, second: 0, third: 6 },\n\
                 Large { first: 0, second: 0, third: 7 },\n\
                 Large { first: 0, second: 0, third: 8 },\n\
                 Large { first: 0, second: 0, third: 42 },\n\
             )\n\
             return value.third\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn allocation_context_crosses_root_and_nested_call_boundaries() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/mem.allocation_context_state_for_test\n\
         use std/mem.allocation_context_kind_for_test\n\
         func leaf(): usize {\n\
             allocation_context_state_for_test() + allocation_context_kind_for_test()\n\
         }\n\
         func middle(): usize { leaf() }\n\
         func main(): i32 {\n\
             if middle() == 0 { return 42 }\n\
             return 1\n\
         }\n",
        &[&["mem"], &["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn pointer_and_view_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.{\n\
             addr, from_ref, pointee_size_for_test, pointee_align_for_test,\n\
         }\n\
         use std/str.{str_len_for_test, str_ptr_addr_for_test}\n\
         use std/slice.{slice_len_for_test, slice_ptr_addr_for_test}\n\
         use std/string.{bytes_from_str_for_test, str_subview_unchecked_for_test}\n\
         func main(): i32 {\n\
             let byte: u8 = 65\n\
             let pointer = from_ref(&byte)\n\
             let address = addr(pointer)\n\
             let middle = str_subview_unchecked_for_test(\"hello\", 1, 3)\n\
             let static_bytes = bytes_from_str_for_test(\"hello\")\n\
             if address == addr(pointer) {\n\
                 if pointee_size_for_test(pointer) == 1 {\n\
                     if pointee_align_for_test(pointer) == 1 {\n\
                         if str_len_for_test(middle) == 3 {\n\
                             if slice_len_for_test(static_bytes) == 5 {\n\
                                 if slice_ptr_addr_for_test(static_bytes)\n\
                                     == str_ptr_addr_for_test(\"hello\") {\n\
                                     return 42\n\
                                 }\n\
                             }\n\
                         }\n\
                     }\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["ptr"], &["str"], &["slice"], &["string"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn memory_transfer_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.{\n\
             copy_ptr_to_ptr_for_test, copy_str_to_ptr_for_test, from_ref, from_ref_mut,\n\
             store_u8_to_ptr_for_test, store_value_to_ptr_for_test,\n\
             take_three_u64_at_ptr_for_test, take_u64_at_ptr_for_test,\n\
         }\n\
         struct Bytes {\n\
             a: u8\n\
             b: u8\n\
             c: u8\n\
             d: u8\n\
             e: u8\n\
         }\n\
         struct Large {\n\
             first: u64\n\
             second: u64\n\
             third: u64\n\
         }\n\
         struct LargePair {\n\
             first: Large\n\
             second: Large\n\
         }\n\
         func main(): i32 {\n\
             var bytes = Bytes { a: 0, b: 0, c: 0, d: 0, e: 0 }\n\
             copy_str_to_ptr_for_test(from_ref_mut(&+bytes.a), 0, \"hello\")\n\
             var copied = Bytes { a: 0, b: 0, c: 0, d: 0, e: 0 }\n\
             copy_ptr_to_ptr_for_test(\n\
                 from_ref_mut(&+copied.a),\n\
                 from_ref(&bytes.a),\n\
                 5,\n\
             )\n\
             store_u8_to_ptr_for_test(from_ref_mut(&+copied.a), 1, 97)\n\
             var pair = LargePair {\n\
                 first: Large { first: 10, second: 20, third: 30 },\n\
                 second: Large { first: 0, second: 0, third: 0 },\n\
             }\n\
             let replacement = Large { first: 40, second: 41, third: 42 }\n\
             let large_pointer = from_ref_mut(&+pair.first)\n\
             store_value_to_ptr_for_test(large_pointer, 24, move replacement)\n\
             let recovered = take_u64_at_ptr_for_test(from_ref(&pair.second.third), 0)\n\
             var arrays: [[u64; 3]; 2] = [[1, 2, 3], [40, 41, 42]]\n\
             let recovered_array = take_three_u64_at_ptr_for_test(\n\
                 from_ref_mut(&+arrays[0]),\n\
                 24,\n\
             )\n\
             if bytes.a == 104 {\n\
                 if bytes.e == 111 {\n\
                     if copied.a == 104 {\n\
                         if copied.b == 97 {\n\
                             if copied.e == 111 {\n\
                                 if recovered == 42 {\n\
                                     if recovered_array[2] == 42 {\n\
                                         return 42\n\
                                     }\n\
                                 }\n\
                             }\n\
                         }\n\
                     }\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn darwin_syscall_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.{syscall0_succeeds_for_test, syscall1_fails_for_test}\n\
         func main(): i32 {\n\
             if syscall0_succeeds_for_test(0x02000014) {\n\
                 if syscall1_fails_for_test(0x02000006, 18446744073709551615) {\n\
                     return 42\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["internal", "os"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn process_exit_primitive_crosses_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         func main(): i32 { exit_for_test(42) }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn trap_and_unreachable_primitives_materialize_without_fallbacks() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.terminate_for_test\n\
         func main(): i32 { terminate_for_test(false) }\n",
        &[&["internal", "os"]],
    );
    let machine = lower_machine_fixture(&fixture);
    nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
}

#[test]
fn allocation_abort_primitive_materializes_without_a_runtime_dependency() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/mem.allocation_abort_for_test\n\
         func main(): i32 { allocation_abort_for_test() }\n",
        &[&["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
}

#[test]
fn user_destruction_and_drop_flags_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         instance ExitOnDrop {\n\
             pub operator (&self == other: &Self): bool { true }\n\
         }\n\
         func main(): i32 {\n\
             let _ = if false {\n\
                 ExitOnDrop { status: 43 } == ExitOnDrop { status: 44 }\n\
             } else {\n\
                 true\n\
             }\n\
             return 42\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);

    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         instance ExitOnDrop {\n\
             pub operator (&self == other: &Self): bool { true }\n\
         }\n\
         func main(): i32 {\n\
             let _ = if true {\n\
                 ExitOnDrop { status: 43 } == ExitOnDrop { status: 44 }\n\
             } else {\n\
                 true\n\
             }\n\
             return 1\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 44);
}

#[test]
fn checked_dynamic_places_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func main(): i32 {\n\
             var values: [i32; 3] = [20, 1, 2]\n\
             values[1] = 20\n\
             return values[0] + values[1] + values[2]\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn built_in_index_borrows_share_native_address_evaluation() {
    let machine = lower_machine(
        "func read(values: &[i32; 3], index: usize): i32 { values[index] }\n\
         func byte_at(text: &str, index: usize): u8 { text[index] }\n\
         func main(): i32 {\n\
             let values: [i32; 3] = [20, 20, 2]\n\
             if byte_at(\"abc\", 1) == 98 {\n\
                 return read(&values, 0) + read(&values, 1) + read(&values, 2)\n\
             }\n\
             return 1\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_and_assert_status(image: &nocter_macho::MachOImage, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);
    let artifact = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "nocter-native-conformance-{}-{artifact}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(status.code(), Some(expected));
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_and_assert_status(_image: &nocter_macho::MachOImage, _expected: i32) {}

fn lower_machine(source: &str) -> MachineProgram {
    let fixture = CompilerFixture::with_app(source);
    lower_machine_fixture(&fixture)
}

fn lower_machine_fixture(fixture: &CompilerFixture) -> MachineProgram {
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (declarations, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, declarations, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared)
        .unwrap()
        .into_parts()
        .0;
    let standard_package = checked.graph().standard_package().unwrap();
    let snapshot = ToolchainSnapshot::select(
        CompilationTarget::Arm64Darwin,
        standard_package,
        primitive_registry(&checked),
    )
    .unwrap();
    let target = TargetProgram::build(checked, snapshot).unwrap();
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    MachineProgram::lower(&lower_executable(executable).unwrap()).unwrap()
}

fn primitive_registry(checked: &nocter_checking::CheckedProgram) -> PrimitiveRegistry {
    let graph = checked.graph();
    PrimitiveRegistry::new(PrimitiveRole::ALL.iter().copied().map(|role| {
        let callable = graph
            .declarations()
            .callables()
            .iter()
            .find_map(|(callable, declaration)| {
                let CallableOwner::Module(module) = declaration.owner() else {
                    return None;
                };
                let actual_path = graph
                    .modules()
                    .get(module)?
                    .path()
                    .segments()
                    .iter()
                    .map(|segment| graph.symbols().spelling(*segment))
                    .collect::<Option<Vec<_>>>()?;
                (declaration.kind() == CallableKind::Primitive
                    && actual_path == role.module_path()
                    && declaration
                        .name()
                        .and_then(|name| graph.symbols().spelling(name))
                        == Some(role.declaration_name()))
                .then_some(callable)
            })
            .unwrap_or_else(|| panic!("missing fixture primitive {role:?}"));
        PrimitiveBinding::new(role, callable)
    }))
    .unwrap()
}
