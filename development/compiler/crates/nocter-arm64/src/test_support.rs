use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_machine::MachineProgram;
use nocter_mir::lower_executable;
use nocter_model::CompilationTarget;
use nocter_runtime_contract::{PrimitiveBinding, PrimitiveRegistry, PrimitiveRole};
use nocter_target_program::{ExecutableProgram, TargetProgram, ToolchainSnapshot};
use nocter_test_support::CompilerFixture;

pub(crate) fn lower_machine(source: &str) -> MachineProgram {
    lower_fixture(&CompilerFixture::with_app(source), false)
}

pub(crate) fn lower_tests(source: &str) -> MachineProgram {
    lower_fixture(&CompilerFixture::with_tests(source), true)
}

fn lower_fixture(fixture: &CompilerFixture, tests: bool) -> MachineProgram {
    let input = fixture.input();
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (declarations, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, declarations, &frontend_bindings, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared)
        .unwrap()
        .into_parts()
        .0;
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = primitive_registry(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
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
    let executable = if tests {
        ExecutableProgram::for_tests(target, selected).unwrap()
    } else {
        ExecutableProgram::for_executable(target, selected).unwrap()
    };
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
                let (module, name) = nocter_test_support::primitive_source_location(role);
                (declaration.kind() == CallableKind::Primitive
                    && actual_path == module
                    && declaration
                        .name()
                        .and_then(|name| graph.symbols().spelling(name))
                        == Some(name))
                .then_some(callable)
            })
            .unwrap_or_else(|| panic!("missing fixture primitive {role:?}"));
        PrimitiveBinding::new(role, callable)
    }))
    .unwrap()
}
