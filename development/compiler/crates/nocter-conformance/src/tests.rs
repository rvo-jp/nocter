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
