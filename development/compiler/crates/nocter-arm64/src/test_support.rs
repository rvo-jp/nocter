use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_machine::MachineProgram;
use nocter_mir::lower_executable;
use nocter_model::CompilationTarget;
use nocter_runtime_contract::{PrimitiveRegistry, RuntimeStorageRegistry};
use nocter_target_program::{ExecutableProgram, TargetProgram, ToolchainSnapshot};
use nocter_test_support::CompilerFixture;

pub(crate) fn lower_machine(source: &str) -> MachineProgram {
    lower_fixture(&CompilerFixture::with_app(source), false)
}

pub(crate) fn lower_machine_with_standard_uses(
    source: &str,
    modules: &[&[&str]],
) -> MachineProgram {
    lower_fixture(
        &CompilerFixture::with_app_standard_uses(source, modules),
        false,
    )
}

pub(crate) fn lower_tests(source: &str) -> MachineProgram {
    lower_fixture(&CompilerFixture::with_tests(source), true)
}

fn lower_fixture(fixture: &CompilerFixture, tests: bool) -> MachineProgram {
    let input = fixture.input();
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let primitive_bindings = lowered.primitive_bindings().to_vec();
    let runtime_storage_bindings = lowered.runtime_storage_bindings().to_vec();
    let (declarations, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, declarations, &frontend_bindings, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared).unwrap();
    let standard_package = checked.program().graph().standard_package().unwrap();
    let snapshot = ToolchainSnapshot::select(
        CompilationTarget::Arm64Darwin,
        standard_package,
        PrimitiveRegistry::new(primitive_bindings).unwrap(),
        nocter_runtime_contract::TargetServiceRegistry::empty(),
        RuntimeStorageRegistry::new(runtime_storage_bindings).unwrap(),
    )
    .unwrap();
    let (checked, _) = checked.into_parts();
    let target = TargetProgram::build(checked, snapshot).unwrap();
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = if tests {
        ExecutableProgram::for_tests(target, selected).unwrap()
    } else {
        ExecutableProgram::for_executable(target, selected).unwrap()
    };
    MachineProgram::lower(&lower_executable(executable).unwrap()).unwrap()
}

/// The private value-access product enters the same Machine/target consumer as other programs.
pub(crate) fn lower_value_machine(source: &str) -> MachineProgram {
    use nocter_checking::value_access::accepted::SourceUnit;
    use nocter_runtime_contract::RuntimeAbiIdentity;
    use nocter_target_program::value_access::{InstanceProgram, lifecycle};
    use std::sync::Arc;
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let mir =
        nocter_mir::value_access::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1)
            .unwrap();
    nocter_machine::value_access::lower(&mir).unwrap()
}
