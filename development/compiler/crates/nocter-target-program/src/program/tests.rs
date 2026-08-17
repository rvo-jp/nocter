use nocter_checking::{
    ConcreteDispatchResolver, GenericArgument, GenericArguments, ResolvedDispatchStep,
    ResolvedPrimitiveDispatch, StaticDispatch, check_prepared_program, prepare_program_checking,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_model::{BuiltinType, CompilationTarget, TypeKind};
use nocter_test_support::CompilerFixture as Fixture;

use super::{TargetProgram, TargetProgramError};
use crate::{
    CallableInstanceKey, CallableInstanceKeyError, EntrySelectionError, PrimitiveBinding,
    PrimitiveContractRule, PrimitiveRegistry, PrimitiveRegistryValidationError, PrimitiveRole,
    ProcessSuccessType, ToolchainSnapshot, collect_body_dependencies, select_executable_entry,
    select_test_target,
};

mod executable;

#[test]
fn complete_closed_registry_constructs_a_target_program() {
    let fixture = Fixture::new();
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
            .unwrap();
    let target = TargetProgram::build(checked, snapshot).unwrap();
    assert_eq!(
        target.checked().graph().target(),
        CompilationTarget::Arm64Darwin
    );
    assert_eq!(target.toolchain().primitives().bindings().len(), 49);
}

#[test]
fn executable_entry_accepts_exactly_the_six_process_result_forms() {
    for (source, expected_success, expected_fallible) in [
        (
            "func main(): void { return }\n",
            ProcessSuccessType::Void,
            false,
        ),
        (
            "func main(): void! { return }\n",
            ProcessSuccessType::Void,
            true,
        ),
        (
            "func main(): i32 { return 0 }\n",
            ProcessSuccessType::I32,
            false,
        ),
        (
            "func main(): i32! { return 0 }\n",
            ProcessSuccessType::I32,
            true,
        ),
        (
            "func main(): usize { return 0 }\n",
            ProcessSuccessType::Usize,
            false,
        ),
        (
            "func main(): usize! { return 0 }\n",
            ProcessSuccessType::Usize,
            true,
        ),
    ] {
        let target = build_target_program(&Fixture::with_app(source));
        let (target_id, _) = target
            .checked()
            .graph()
            .package_targets()
            .iter()
            .next()
            .unwrap();
        let entry = select_executable_entry(&target, target_id).unwrap();
        assert_eq!(entry.process_result().success(), expected_success);
        assert_eq!(entry.process_result().is_fallible(), expected_fallible);
        assert_eq!(entry.target(), target_id);
        assert_eq!(
            target
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(entry.callable())
                .and_then(nocter_declarations::CallableDeclaration::body),
            Some(entry.body())
        );
    }
}

#[test]
fn executable_entry_rejects_missing_non_function_and_invalid_callable_contracts() {
    let cases = [
        ("", None),
        ("struct main {}\n", None),
        (
            "func main<T>(): void { return }\n",
            Some(crate::EntryContractRule::GenericParameters),
        ),
        (
            "func main(value: i32): void { return }\n",
            Some(crate::EntryContractRule::ValueParameters),
        ),
        (
            "func main(): u64 { return 0 }\n",
            Some(crate::EntryContractRule::ResultType),
        ),
    ];
    for (source, expected_rule) in cases {
        let target = build_target_program(&Fixture::with_app(source));
        let (target_id, _) = target
            .checked()
            .graph()
            .package_targets()
            .iter()
            .next()
            .unwrap();
        let error = select_executable_entry(&target, target_id).unwrap_err();
        match (error, expected_rule) {
            (
                EntrySelectionError::MissingMain { .. }
                | EntrySelectionError::InvalidMainEntity { .. },
                None,
            ) => {}
            (EntrySelectionError::InvalidMainContract { rule: actual, .. }, Some(expected)) => {
                assert_eq!(actual, expected);
            }
            _ => panic!("unexpected entry-selection result for {source:?}"),
        }
    }
}

#[test]
fn callable_instance_key_requires_the_complete_concrete_generic_domain() {
    let target = build_target_program(&Fixture::with_app(
        "func helper<T>(value: T): T { return move value }\n\
         func main(): void { return }\n",
    ));
    let graph = target.checked().graph();
    let helper = graph
        .declarations()
        .callables()
        .iter()
        .find_map(|(id, declaration)| {
            (declaration
                .name()
                .and_then(|name| graph.symbols().spelling(name))
                == Some("helper"))
            .then_some(id)
        })
        .unwrap();
    let parameter = graph
        .declarations()
        .callables()
        .get(helper)
        .unwrap()
        .generic_parameters()[0];
    let concrete = target.checked().types().builtin(BuiltinType::I32);
    let key = CallableInstanceKey::new(
        &target,
        helper,
        GenericArguments::new([GenericArgument::new(parameter, concrete)]).unwrap(),
    )
    .unwrap();
    assert_eq!(key.callable(), helper);
    assert_eq!(key.generic_arguments().get(parameter), Some(concrete));

    assert!(matches!(
        CallableInstanceKey::new(&target, helper, GenericArguments::default()),
        Err(CallableInstanceKeyError::GenericDomainMismatch { .. })
    ));
    let symbolic = target
        .checked()
        .types()
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, TypeKind::GenericParameter(actual) if *actual == parameter).then_some(id)
        })
        .unwrap();
    assert!(matches!(
        CallableInstanceKey::new(
            &target,
            helper,
            GenericArguments::new([GenericArgument::new(parameter, symbolic)]).unwrap(),
        ),
        Err(CallableInstanceKeyError::SymbolicArgument { .. })
    ));
}

#[test]
fn body_dependencies_follow_only_executable_checked_edges() {
    let target = build_target_program(&Fixture::with_app(
        "func live(): void { return }\n\
         func dead(): void { return }\n\
         func main(): void {\n\
             live()\n\
             return\n\
             dead()\n\
         }\n",
    ));
    let graph = target.checked().graph();
    let callable_named = |expected: &str| {
        graph
            .declarations()
            .callables()
            .iter()
            .find_map(|(id, declaration)| {
                (declaration
                    .name()
                    .and_then(|name| graph.symbols().spelling(name))
                    == Some(expected))
                .then_some(id)
            })
            .unwrap()
    };
    let main = callable_named("main");
    let live = callable_named("live");
    let dead = callable_named("dead");
    let body = graph
        .declarations()
        .callables()
        .get(main)
        .unwrap()
        .body()
        .unwrap();
    let root = target.checked().bodies().get(body).unwrap().root();
    let dependencies = collect_body_dependencies(&target, body, root).unwrap();
    let direct = dependencies
        .selections()
        .iter()
        .filter_map(|selection| match selection.dispatch() {
            StaticDispatch::Direct(callable) => Some(callable),
            StaticDispatch::InterfaceMethod { .. }
            | StaticDispatch::OpaqueMethod { .. }
            | StaticDispatch::StructuralRequirement(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(direct.contains(&live));
    assert!(!direct.contains(&dead));
}

#[test]
fn concrete_dispatch_resolves_a_generic_structural_comparison_to_a_primitive() {
    let target = build_target_program(&Fixture::with_app(
        "func equal<T>(left: T, right: T): bool where (&T == &T): bool {\n\
             return left == right\n\
         }\n\
         func main(): void {\n\
             let _ = equal(1, 2)\n\
             return\n\
         }\n",
    ));
    let main = named_callable(&target, "main");
    let equal = named_callable(&target, "equal");
    let main_dependencies = callable_dependencies(&target, main);
    let equal_selection = main_dependencies
        .selections()
        .iter()
        .find(|selection| selection.dispatch() == StaticDispatch::Direct(equal))
        .unwrap();
    let equal_key =
        CallableInstanceKey::new(&target, equal, equal_selection.generic_arguments().clone())
            .unwrap();
    let equal_dependencies = callable_dependencies(&target, equal);
    let structural = equal_dependencies
        .selections()
        .iter()
        .find(|selection| {
            matches!(
                selection.dispatch(),
                StaticDispatch::StructuralRequirement(_)
            )
        })
        .unwrap();
    let module = callable_module(&target, equal);
    let mut resolver = ConcreteDispatchResolver::new(target.checked());
    let plan = resolver
        .resolve(structural, &equal_key.substitution(), module)
        .unwrap();

    assert_eq!(
        plan.steps(),
        [ResolvedDispatchStep::Primitive(
            ResolvedPrimitiveDispatch::Equality(target.checked().types().builtin(BuiltinType::I32))
        )]
    );
}

#[test]
fn concrete_dispatch_maps_a_lexical_interface_method_to_its_conformance_body() {
    let target = build_target_program(&Fixture::with_app(
        "pub interface Readable {\n\
             pub method &self.read(): i32\n\
         }\n\
         struct Value {}\n\
         conform Readable for Value {\n\
             method &self.read(): i32 { return 42 }\n\
         }\n\
         func generic<T>(input: &T): i32 where T: Readable {\n\
             return input.read()\n\
         }\n\
         func main(): i32 {\n\
             let value = Value {}\n\
             return generic(&value)\n\
         }\n",
    ));
    let graph = target.checked().graph();
    let generic = named_callable(&target, "generic");
    let main = named_callable(&target, "main");
    let main_dependencies = callable_dependencies(&target, main);
    let generic_selection = main_dependencies
        .selections()
        .iter()
        .find(|selection| selection.dispatch() == StaticDispatch::Direct(generic))
        .unwrap();
    let generic_key = CallableInstanceKey::new(
        &target,
        generic,
        generic_selection.generic_arguments().clone(),
    )
    .unwrap();
    let generic_dependencies = callable_dependencies(&target, generic);
    let interface_selection = generic_dependencies
        .selections()
        .iter()
        .find(|selection| matches!(selection.dispatch(), StaticDispatch::InterfaceMethod { .. }))
        .unwrap();
    let module = callable_module(&target, generic);
    let mut dispatch_resolver = ConcreteDispatchResolver::new(target.checked());
    let plan = dispatch_resolver
        .resolve(interface_selection, &generic_key.substitution(), module)
        .unwrap();
    let [ResolvedDispatchStep::Direct(method_dispatch)] = plan.steps() else {
        panic!("interface dispatch must resolve to one conformance method")
    };

    assert!(matches!(
        graph
            .declarations()
            .callables()
            .get(method_dispatch.callable())
            .unwrap()
            .owner(),
        CallableOwner::Conformance(_)
    ));
    assert!(method_dispatch.generic_arguments().as_slice().is_empty());
}

#[test]
fn concrete_dispatch_opens_an_opaque_witness_only_during_specialization() {
    let target = build_target_program(&Fixture::with_app(
        "pub interface Readable {\n\
             pub method &self.read(): i32\n\
         }\n\
         struct Value {}\n\
         conform Readable for Value {\n\
             method &self.read(): i32 { 42 }\n\
         }\n\
         func hide<T>(value: T): some Readable where T: Readable { move value }\n\
         func main(): i32 { hide(Value {}).read() }\n",
    ));
    let main = named_callable(&target, "main");
    let dependencies = callable_dependencies(&target, main);
    let selection = dependencies
        .selections()
        .iter()
        .find(|selection| matches!(selection.dispatch(), StaticDispatch::OpaqueMethod { .. }))
        .unwrap();
    let mut resolver = ConcreteDispatchResolver::new(target.checked());
    let plan = resolver
        .resolve(
            selection,
            &nocter_checking::TypeSubstitution::default(),
            callable_module(&target, main),
        )
        .unwrap();
    let [ResolvedDispatchStep::Direct(method)] = plan.steps() else {
        panic!("opaque dispatch must resolve to one conformance method")
    };

    assert!(matches!(
        target
            .checked()
            .graph()
            .declarations()
            .callables()
            .get(method.callable())
            .unwrap()
            .owner(),
        CallableOwner::Conformance(_)
    ));
}

#[test]
fn test_target_selects_only_direct_cases_in_source_order() {
    let target = build_target_program(&Fixture::with_tests(
        "test first { return }\n\
         test second { return }\n",
    ));
    let (target_id, _) = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap();
    let selected = select_test_target(&target, target_id).unwrap();
    assert_eq!(selected.target(), target_id);
    assert_eq!(selected.tests().len(), 2);
    assert_eq!(
        selected
            .tests()
            .iter()
            .map(|test| target
                .checked()
                .graph()
                .symbols()
                .spelling(test.name())
                .unwrap())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    for test in selected.tests() {
        assert_eq!(
            target
                .checked()
                .graph()
                .declarations()
                .tests()
                .get(test.declaration())
                .copied()
                .map(nocter_declarations::TestDeclaration::body),
            Some(test.body())
        );
    }
}

#[test]
fn semantic_attachment_cannot_swap_same_shaped_primitive_names() {
    let fixture = Fixture::new();
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let mut bindings = registry.bindings().to_vec();
    let left = bindings
        .iter()
        .position(|binding| binding.role() == PrimitiveRole::CurrentAllocatorState)
        .unwrap();
    let right = bindings
        .iter()
        .position(|binding| binding.role() == PrimitiveRole::CurrentAllocatorKind)
        .unwrap();
    let left_callable = bindings[left].callable();
    let right_callable = bindings[right].callable();
    bindings[left] = PrimitiveBinding::new(bindings[left].role(), right_callable);
    bindings[right] = PrimitiveBinding::new(bindings[right].role(), left_callable);
    let snapshot = ToolchainSnapshot::select(
        CompilationTarget::Arm64Darwin,
        standard_package,
        PrimitiveRegistry::new(bindings).unwrap(),
    )
    .unwrap();
    let error = TargetProgram::build(checked, snapshot).unwrap_err();
    let TargetProgramError::PrimitiveRegistry(PrimitiveRegistryValidationError::Contract(error)) =
        error
    else {
        panic!("unexpected target-program error")
    };
    assert_eq!(error.role(), PrimitiveRole::CurrentAllocatorState);
    assert_eq!(error.rule(), PrimitiveContractRule::Name);
}

fn build_target_program(fixture: &Fixture) -> TargetProgram {
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
            .unwrap();
    TargetProgram::build(checked, snapshot).unwrap()
}

fn named_callable(target: &TargetProgram, expected: &str) -> nocter_model::CallableId {
    let graph = target.checked().graph();
    graph
        .declarations()
        .callables()
        .iter()
        .find_map(|(id, declaration)| {
            (declaration
                .name()
                .and_then(|name| graph.symbols().spelling(name))
                == Some(expected))
            .then_some(id)
        })
        .unwrap_or_else(|| panic!("missing fixture callable {expected}"))
}

fn callable_dependencies(
    target: &TargetProgram,
    callable: nocter_model::CallableId,
) -> crate::CheckedBodyDependencies {
    let body = target
        .checked()
        .graph()
        .declarations()
        .callables()
        .get(callable)
        .and_then(nocter_declarations::CallableDeclaration::body)
        .unwrap();
    collect_body_dependencies(
        target,
        body,
        target.checked().bodies().get(body).unwrap().root(),
    )
    .unwrap()
}

fn callable_module(
    target: &TargetProgram,
    callable: nocter_model::CallableId,
) -> nocter_model::ModuleId {
    let CallableOwner::Module(module) = target
        .checked()
        .graph()
        .declarations()
        .callables()
        .get(callable)
        .unwrap()
        .owner()
    else {
        panic!("fixture callable must be module-owned")
    };
    module
}

fn registry_for(checked: &nocter_checking::CheckedProgram) -> PrimitiveRegistry {
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
