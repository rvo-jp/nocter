use nocter_checking::{CleanupTarget, ConcreteDestructionKind, StaticDispatch};
use nocter_declarations::CallableOwner;
use nocter_model::BuiltinType;

use super::{Fixture, build_target_program, callable_dependencies, named_callable};
use crate::{
    ExecutableDispatchStep, ExecutableItemKey, ExecutableProgram, ExecutableRoot, PrimitiveRole,
};

#[test]
fn executable_closure_contains_only_reachable_bodies_and_recursive_drop_work() {
    let target = build_target_program(&Fixture::with_app(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func live(): void { return }\n\
         func dead(): void { return }\n\
         func main(): void {\n\
             live()\n\
             let value = Owned { value: 1 }\n\
             let callback = (move value;): void { return }\n\
             callback()\n\
             return\n\
         }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let names = executable
        .items()
        .iter()
        .filter_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) => executable
                .target()
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(key.callable())
                .and_then(nocter_declarations::CallableDeclaration::name)
                .and_then(|name| {
                    executable
                        .target()
                        .checked()
                        .graph()
                        .symbols()
                        .spelling(name)
                }),
            ExecutableItemKey::Closure(_)
            | ExecutableItemKey::Drop(_)
            | ExecutableItemKey::Test(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&"main"));
    assert!(names.contains(&"live"));
    assert!(!names.contains(&"dead"));
    assert_eq!(
        executable
            .items()
            .iter()
            .filter(|(_, item)| matches!(item.key(), ExecutableItemKey::Closure(_)))
            .count(),
        1
    );
    let keys = executable
        .items()
        .iter()
        .map(|(_, item)| item.key().clone())
        .collect::<Vec<_>>();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    assert_eq!(
        executable
            .items()
            .iter()
            .filter(|(_, item)| matches!(item.key(), ExecutableItemKey::Drop(_)))
            .count(),
        1
    );

    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("executable selection must produce a process root")
    };
    let main = executable.items().get(*entry).unwrap();
    assert_eq!(main.body().closures().len(), 1);
    assert_eq!(main.body().drops().len(), 1);
    let closure_type = main
        .body()
        .types()
        .iter()
        .find_map(|edge| {
            matches!(
                executable.types().get(edge.concrete()),
                Some(nocter_model::TypeKind::Closure { .. })
            )
            .then_some(edge.source())
        })
        .unwrap();
    assert!(
        main.body()
            .complete_destruction_for_source(closure_type)
            .is_some()
    );
}

#[test]
fn closure_and_drop_keys_inherit_the_complete_concrete_owner_domain() {
    let target = build_target_program(&Fixture::with_app(
        "struct Owned<T> { value: T }\n\
         drop Owned<T>(&+self) { return }\n\
         func run<T>(value: Owned<T>): void {\n\
             let callback = (move value;): void { return }\n\
             callback()\n\
             return\n\
         }\n\
         func main(): void {\n\
             run(Owned { value: 1 })\n\
             return\n\
         }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let i32_ = executable.types().builtin(BuiltinType::I32);
    let closure = executable
        .items()
        .iter()
        .find_map(|(_, item)| match item.key() {
            ExecutableItemKey::Closure(key) => Some(key),
            ExecutableItemKey::Callable(_)
            | ExecutableItemKey::Drop(_)
            | ExecutableItemKey::Test(_) => None,
        })
        .unwrap();
    let drop = executable
        .items()
        .iter()
        .find_map(|(_, item)| match item.key() {
            ExecutableItemKey::Drop(key) => Some(key),
            ExecutableItemKey::Callable(_)
            | ExecutableItemKey::Closure(_)
            | ExecutableItemKey::Test(_) => None,
        })
        .unwrap();

    assert_eq!(closure.generic_arguments().as_slice()[0].ty(), i32_);
    assert_eq!(drop.generic_arguments().as_slice()[0].ty(), i32_);
}

#[test]
fn generic_direct_dispatch_names_the_dense_specialized_item() {
    let target = build_target_program(&Fixture::with_app(
        "func identity<T>(value: T): T { move value }\n\
         func main(): i32 { identity(7) }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let main = named_callable(executable.target(), "main");
    let identity = named_callable(executable.target(), "identity");
    let selection = callable_dependencies(executable.target(), main)
        .selections()
        .iter()
        .find(|selection| selection.dispatch() == StaticDispatch::Direct(identity))
        .unwrap()
        .clone();
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let plan = executable
        .items()
        .get(*entry)
        .unwrap()
        .body()
        .dispatch(&selection)
        .unwrap();
    let [ExecutableDispatchStep::Direct(identity_item)] = plan.steps() else {
        panic!("generic direct call must name one dense executable item")
    };
    let ExecutableItemKey::Callable(key) = executable.items().get(*identity_item).unwrap().key()
    else {
        panic!("direct target must be a callable specialization")
    };

    assert_eq!(key.callable(), identity);
    assert_eq!(
        key.generic_arguments().as_slice()[0].ty(),
        executable.types().builtin(BuiltinType::I32)
    );
}

#[test]
fn executable_cleanup_keeps_enum_residual_distinct_from_complete_destruction() {
    let target = build_target_program(&Fixture::with_app(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         drop Pair(&+self) { return }\n\
         func main(): void {\n\
             let retained = match Pair.values(Owned { value: 1 }, Owned { value: 2 }) {\n\
                 Pair.values(item, _) { move item }\n\
             }\n\
             return\n\
         }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let item = executable.items().get(*entry).unwrap();
    let checked = executable
        .target()
        .checked()
        .bodies()
        .get(item.body().body())
        .unwrap();
    let residual = checked
        .nodes()
        .iter()
        .flat_map(|(node, _)| checked.cleanups().schedules(node).into_iter().flatten())
        .flat_map(nocter_checking::CleanupSchedule::actions)
        .find_map(|action| match action.target() {
            target @ CleanupTarget::EnumResidual { .. } => Some(target),
            CleanupTarget::Path(_)
            | CleanupTarget::Place { .. }
            | CleanupTarget::Value { .. }
            | CleanupTarget::Region { .. } => None,
        })
        .unwrap();
    let plan = item.body().cleanup_destruction(residual).unwrap();
    let ConcreteDestructionKind::Enum { drop, variants } = plan.kind() else {
        panic!("residual cleanup must preserve enum variant selection")
    };

    assert!(drop.is_none());
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].payload().len(), 1);
}

#[test]
fn bodyless_standard_calls_become_typed_primitive_steps() {
    let target = build_target_program(&Fixture::with_app_standard_uses(
        "use std/ptr.addr\n\
         use std/ptr.from_ref\n\
         func main(): usize {\n\
             let value = 1\n\
             addr(from_ref(&value))\n\
         }\n",
        &[&["ptr"], &["ptr"]],
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let main = named_callable(executable.target(), "main");
    let dependencies = callable_dependencies(executable.target(), main);
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let body = executable.items().get(*entry).unwrap().body();
    let roles = dependencies
        .selections()
        .iter()
        .flat_map(|selection| body.dispatch(selection).unwrap().steps())
        .filter_map(|step| match step {
            ExecutableDispatchStep::StandardPrimitive(call) => Some(call.role()),
            ExecutableDispatchStep::Direct(_)
            | ExecutableDispatchStep::StructuralPrimitive(_)
            | ExecutableDispatchStep::IndirectCallable(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(roles.contains(&PrimitiveRole::PointerFromReference));
    assert!(roles.contains(&PrimitiveRole::PointerAddress));
    assert_eq!(executable.items().len(), 1);
}

#[test]
fn interface_dispatch_enqueues_only_the_selected_conformance_body() {
    let target = build_target_program(&Fixture::with_app(
        "pub interface Readable { pub method &self.read(): i32 }\n\
         struct Value {}\n\
         conform Readable for Value {\n\
             method &self.read(): i32 { 42 }\n\
         }\n\
         func read<T>(value: &T): i32 where T: Readable { value.read() }\n\
         func main(): i32 {\n\
             let value = Value {}\n\
             read(&value)\n\
         }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
    let owners = executable
        .items()
        .iter()
        .filter_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) => executable
                .target()
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(key.callable())
                .map(nocter_declarations::CallableDeclaration::owner),
            ExecutableItemKey::Closure(_)
            | ExecutableItemKey::Drop(_)
            | ExecutableItemKey::Test(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(
        owners
            .iter()
            .any(|owner| matches!(owner, CallableOwner::Conformance(_)))
    );
    assert!(
        !owners
            .iter()
            .any(|owner| matches!(owner, CallableOwner::Interface(_)))
    );
}

#[test]
fn test_root_preserves_selected_case_order_without_scanning_unreachable_functions() {
    let target = build_target_program(&Fixture::with_tests(
        "func shared(): void { return }\n\
         func unused(): void { return }\n\
         test first {\n\
             shared()\n\
             return\n\
         }\n\
         test second { return }\n",
    ));
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_tests(target, selected).unwrap();
    let ExecutableRoot::Tests { cases, .. } = executable.root() else {
        panic!("test selection must produce a test root")
    };
    let graph = executable.target().checked().graph();

    assert_eq!(
        cases
            .iter()
            .map(|case| graph.symbols().spelling(case.name()).unwrap())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        cases
            .iter()
            .all(|case| executable.items().get(case.item()).is_some())
    );
    assert!(executable.items().iter().any(|(_, item)| {
        matches!(item.key(), ExecutableItemKey::Callable(key) if graph
            .declarations()
            .callables()
            .get(key.callable())
            .is_some_and(|callable| callable.owner() == CallableOwner::Module(
                graph.package_targets().get(selected).unwrap().module()
            ) && callable.name().and_then(|name| graph.symbols().spelling(name)) == Some("shared")))
    }));
    assert!(!executable.items().iter().any(|(_, item)| {
        matches!(item.key(), ExecutableItemKey::Callable(key) if graph
            .declarations()
            .callables()
            .get(key.callable())
            .and_then(nocter_declarations::CallableDeclaration::name)
            .and_then(|name| graph.symbols().spelling(name)) == Some("unused"))
    }));
}
