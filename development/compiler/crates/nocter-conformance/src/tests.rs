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
