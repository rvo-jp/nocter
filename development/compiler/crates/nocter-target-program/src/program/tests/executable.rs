use std::sync::Arc;

use nocter_checking::{CleanupTarget, ConcreteDestructionKind, SpreadMode, StaticDispatch};
use nocter_declarations::{CallableKind, CallableOwner, LiteralShape};
use nocter_model::BuiltinType;
use nocter_runtime_contract::{PrimitiveRole, RuntimeTypeRepresentation};

use super::{Fixture, build_target_program, callable_dependencies, named_callable};
use crate::{
    ExecutableDispatchPlan, ExecutableDispatchStep, ExecutableInputSource, ExecutableItemKey,
    ExecutablePackSegment, ExecutablePrimitiveDependency, ExecutableProgram, ExecutableRoot,
    ExecutableTestCase,
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
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let dead_body = target
        .checked()
        .graph()
        .declarations()
        .callables()
        .get(named_callable(&target, "dead"))
        .and_then(nocter_declarations::CallableDeclaration::body)
        .unwrap();
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    assert_reached_bodies_are_frozen(&executable, dead_body);
    let names = executable
        .items()
        .iter()
        .filter_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) => target
                .as_ref()
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(key.callable())
                .and_then(nocter_declarations::CallableDeclaration::name)
                .and_then(|name| target.as_ref().checked().graph().symbols().spelling(name)),
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

fn assert_reached_bodies_are_frozen(
    executable: &ExecutableProgram,
    unreachable: nocter_model::BodyId,
) {
    assert!(executable.checked_body(unreachable).is_none());
    assert!(
        executable
            .items()
            .iter()
            .all(|(_, item)| executable.checked_body(item.body().body()).is_some())
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
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
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
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let main = named_callable(target.as_ref(), "main");
    let identity = named_callable(target.as_ref(), "identity");
    let selection = callable_dependencies(target.as_ref(), main)
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
    let ExecutableDispatchPlan::Invocation(ExecutableDispatchStep::Direct(identity_item)) = plan
    else {
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
fn generic_associated_results_reduce_through_the_selected_implementation() {
    let target = build_target_program(&Fixture::with_app(
        "pub interface Source {\n\
             pub type Item\n\
             pub method &self.get(): Self.Item\n\
         }\n\
         struct Buffer {}\n\
         instance Buffer {\n\
             impl Source { .Item = i32 }\n\
             method &self.get(): i32 { 7 }\n\
         }\n\
         func read<S>(source: &S): S.Item where S impl Source { source.get() }\n\
         func main(): i32 {\n\
             let source = Buffer {}\n\
             read(&source)\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let read = named_callable(target.as_ref(), "read");
    let item = executable
        .items()
        .iter()
        .find_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) if key.callable() == read => Some(item),
            _ => None,
        })
        .expect("generic associated-result specialization");

    assert_eq!(
        item.signature().result(),
        executable.types().builtin(BuiltinType::I32)
    );
}

#[test]
fn callable_bound_dispatch_freezes_the_concrete_closure_body_and_cleanup() {
    let target = build_target_program(&Fixture::with_app(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func finish<F>(callback: F): i32 where F: func(): i32 { callback() }\n\
         func main(): i32 {\n\
             let value = Owned { value: 7 }\n\
             finish((move value;): i32 { value.value })\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let finish = named_callable(&target, "finish");
    let selection = callable_dependencies(&target, finish).selections()[0].clone();
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let finish_item = executable
        .items()
        .iter()
        .find_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) if key.callable() == finish => Some(item),
            _ => None,
        })
        .unwrap();
    let ExecutableDispatchPlan::Invocation(ExecutableDispatchStep::CallableValue(invocation)) =
        finish_item.body().dispatch(&selection).unwrap()
    else {
        panic!("callable bound must freeze one concrete invocation")
    };
    let body = executable.items().get(invocation.body()).unwrap();
    let layout = body.closure_layout().unwrap();

    assert_eq!(invocation.subject(), layout.ty());
    assert_eq!(
        invocation.contract().capability(),
        nocter_model::CallableCapability::Owned
    );
    assert_eq!(
        layout.capability(),
        nocter_model::CallableCapability::Readonly
    );
    assert!(invocation.post_call_destruction().is_some());
    assert!(matches!(body.key(), ExecutableItemKey::Closure(_)));
}

#[test]
fn executable_signature_specializes_even_unused_parameters() {
    let target = build_target_program(&Fixture::with_app(
        "func constant<T>(unused: T): i32 { 7 }\n\
         func main(): i32 { constant(1) }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let constant = named_callable(target.as_ref(), "constant");
    let declaration = target
        .as_ref()
        .checked()
        .graph()
        .declarations()
        .callables()
        .get(constant)
        .unwrap();
    let item = executable
        .items()
        .iter()
        .find_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) if key.callable() == constant => Some(item),
            _ => None,
        })
        .unwrap();
    let i32_ = executable.types().builtin(BuiltinType::I32);

    assert_eq!(item.signature().inputs().len(), 1);
    assert_eq!(
        item.signature().inputs()[0].source(),
        ExecutableInputSource::Parameter(declaration.parameters()[0])
    );
    assert_eq!(item.signature().inputs()[0].ty(), i32_);
    assert_eq!(item.signature().result(), i32_);
}

#[test]
fn literal_argument_pack_is_not_an_ordinary_executable_input() {
    let target = build_target_program(&Fixture::with_app(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self { return Self {} }\n\
         }\n\
         func main(): i32 {\n\
             let values = Vec [1, 2]\n\
             drop values\n\
             0\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let (literal_id, declaration, item) = executable
        .items()
        .iter()
        .find_map(|(item_id, item)| {
            let ExecutableItemKey::Callable(key) = item.key() else {
                return None;
            };
            let declaration = target
                .as_ref()
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(key.callable())?;
            (declaration.kind() == CallableKind::Literal(LiteralShape::Sequence)).then_some((
                item_id,
                declaration,
                item,
            ))
        })
        .unwrap();
    let pack = item.signature().pack().unwrap();

    assert!(item.signature().inputs().is_empty());
    assert_eq!(pack.source(), declaration.parameters()[0]);
    assert_eq!(pack.element(), executable.types().builtin(BuiltinType::I32));
    assert!(matches!(
        executable.types().get(pack.next()),
        Some(nocter_model::TypeKind::Optional(payload)) if *payload == pack.element()
    ));

    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let main = executable.items().get(*entry).unwrap();
    let [plan] = main.body().pack_literals() else {
        panic!("expected one pack-literal plan")
    };
    assert_eq!(main.body().pack_literal(plan.source()), Some(plan));
    assert_eq!(plan.constructor(), literal_id);
    assert_eq!(plan.input(), pack);
    assert_eq!(plan.result(), item.signature().result());
    assert!(matches!(
        plan.segments(),
        [
            ExecutablePackSegment::Value { ty: left, .. },
            ExecutablePackSegment::Value { ty: right, .. },
        ] if *left == pack.element() && *right == pack.element()
    ));
}

#[test]
fn pack_literal_plan_freezes_spread_types_and_dispatch_in_source_order() {
    let fixture = Fixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self { return Self {} }\n\
         }\n\
         struct Item {}\n\
         drop Item(&+self) { return }\n\
         struct Iter {}\n\
         instance Iter {\n\
             impl Iterator { .Item = Item }\n\
             method &+self.next(): Item? { return none }\n\
         }\n\
         instance Iter {\n\
             impl ExactSizeIterator\n\
             method &self.remaining_len(): usize { return 0 }\n\
         }\n\
         drop Iter(&+self) { return }\n\
         func main(): i32 {\n\
             let iterator = Iter {}\n\
             let values = Vec [Item {}, ...move iterator, Item {}]\n\
             drop values\n\
             0\n\
         }\n",
        &[&[], &[]],
    );
    let target = build_target_program(&fixture);
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let main = executable.items().get(*entry).unwrap();
    let [plan] = main.body().pack_literals() else {
        panic!("expected one pack-literal plan")
    };
    let [
        ExecutablePackSegment::Value {
            ty: first,
            destruction: first_destruction,
            ..
        },
        ExecutablePackSegment::Spread(spread),
        ExecutablePackSegment::Value {
            ty: last,
            destruction: last_destruction,
            ..
        },
    ] = plan.segments()
    else {
        panic!("expected fixed, spread, fixed segment order")
    };

    assert_eq!(*first, plan.input().element());
    assert_eq!(*last, plan.input().element());
    assert!(first_destruction.is_some());
    assert!(last_destruction.is_some());
    assert_eq!(spread.mode(), SpreadMode::Move);
    assert_eq!(spread.item(), plan.input().element());
    assert_eq!(spread.contribution(), plan.input().element());
    assert!(spread.destruction().is_some());
    assert_eq!(main.body().drops().len(), 2);
    assert!(matches!(
        main.body().dispatch(spread.next()),
        Some(ExecutableDispatchPlan::Invocation(_))
    ));
    assert!(matches!(
        main.body().dispatch(spread.exact_size()),
        Some(ExecutableDispatchPlan::Invocation(_))
    ));
}

#[test]
fn executable_signatures_materialize_receiver_capabilities() {
    let target = build_target_program(&Fixture::with_app(
        "struct Value { field: i32 }\n\
         drop Value(&+self) { return }\n\
         instance Value { pub method &self.read(): i32 { self.field } }\n\
         func main(): i32 {\n\
             let value = Value { field: 7 }\n\
             value.read()\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let mut saw_method = false;
    let mut saw_drop = false;
    for (_, item) in executable.items().iter() {
        let expected = match item.key() {
            ExecutableItemKey::Callable(key)
                if matches!(
                    target
                        .as_ref()
                        .checked()
                        .graph()
                        .declarations()
                        .callables()
                        .get(key.callable())
                        .unwrap()
                        .owner(),
                    CallableOwner::Instance(_)
                ) =>
            {
                saw_method = true;
                Some(nocter_model::BorrowCapability::Readonly)
            }
            ExecutableItemKey::Drop(_) => {
                saw_drop = true;
                Some(nocter_model::BorrowCapability::ReadWrite)
            }
            _ => None,
        };
        if let Some(expected) = expected {
            assert!(matches!(
                executable.types().get(item.signature().inputs()[0].ty()),
                Some(nocter_model::TypeKind::Borrow { capability, .. }) if *capability == expected
            ));
        }
    }
    assert!(saw_method && saw_drop);
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
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let item = executable.items().get(*entry).unwrap();
    let checked = target
        .as_ref()
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
        "use std/ptr\n\
         func main(): usize {\n\
             let value = 1\n\
             ptr.addr(ptr.from_ref(&value))\n\
         }\n",
        &[&["ptr"]],
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let main = named_callable(target.as_ref(), "main");
    let dependencies = callable_dependencies(target.as_ref(), main);
    let ExecutableRoot::Process { entry, .. } = executable.root() else {
        panic!("expected process root")
    };
    let body = executable.items().get(*entry).unwrap().body();
    let roles = dependencies
        .selections()
        .iter()
        .flat_map(|selection| dispatch_steps(body.dispatch(selection).unwrap()))
        .filter_map(|step| match step {
            ExecutableDispatchStep::StandardPrimitive(call) => Some(call.role()),
            ExecutableDispatchStep::Direct(_)
            | ExecutableDispatchStep::StructuralPrimitive(_)
            | ExecutableDispatchStep::CallableValue(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(roles.contains(&PrimitiveRole::PointerFromReference));
    assert!(roles.contains(&PrimitiveRole::PointerAddress));
    assert_eq!(executable.items().len(), 1);
    for primitive in dependencies
        .selections()
        .iter()
        .flat_map(|selection| dispatch_steps(body.dispatch(selection).unwrap()))
        .filter_map(|step| match step {
            ExecutableDispatchStep::StandardPrimitive(call) => Some(call),
            _ => None,
        })
    {
        assert!(!primitive.signature().inputs().is_empty());
        assert!(
            executable
                .types()
                .get(primitive.signature().result())
                .is_some()
        );
    }
}

#[test]
fn pointer_destruction_primitive_freezes_its_concrete_semantic_dependency() {
    let target = build_target_program(&Fixture::with_app_standard_uses(
        "use std/internal/ptr as internal_ptr\n\
         use std/ptr\n\
         struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         func main(): i32 {\n\
             var value = Resource {}\n\
             let pointer = ptr.from_ref_mut(&+value)\n\
             internal_ptr.drop_value_at_ptr_for_test(pointer, 0)\n\
             return 0\n\
         }\n",
        &[&["internal", "ptr"], &["ptr"]],
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let wrapper = named_callable(&target, "drop_value_at_ptr_for_test");
    let dependencies = callable_dependencies(&target, wrapper);
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let body = executable
        .items()
        .iter()
        .find_map(|(_, item)| {
            matches!(item.key(), ExecutableItemKey::Callable(key) if key.callable() == wrapper)
                .then_some(item.body())
        })
        .expect("reachable concrete wrapper");
    let primitive = dependencies
        .selections()
        .iter()
        .flat_map(|selection| dispatch_steps(body.dispatch(selection).unwrap()))
        .find_map(|step| match step {
            ExecutableDispatchStep::StandardPrimitive(call)
                if call.role() == PrimitiveRole::DropValueAtPointer =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("drop-value primitive");
    let ExecutablePrimitiveDependency::Destruction {
        subject,
        plan: Some(plan),
    } = primitive.dependency()
    else {
        panic!("drop-value primitive must retain concrete destruction")
    };

    assert_eq!(plan.ty(), *subject);
    assert!(matches!(
        plan.kind(),
        ConcreteDestructionKind::Struct {
            drop: Some(_),
            fields,
        } if fields.is_empty()
    ));
    assert!(
        executable
            .items()
            .iter()
            .any(|(_, item)| matches!(item.key(), ExecutableItemKey::Drop(_)))
    );
}

fn dispatch_steps(plan: &ExecutableDispatchPlan) -> Vec<&ExecutableDispatchStep> {
    match plan {
        ExecutableDispatchPlan::Invocation(step)
        | ExecutableDispatchPlan::OpaqueInvocation {
            operation: step, ..
        } => vec![step],
        ExecutableDispatchPlan::Comparison {
            left_coercion,
            right_coercion,
            operation,
        } => left_coercion
            .iter()
            .chain(right_coercion)
            .chain([operation])
            .collect(),
        ExecutableDispatchPlan::Index {
            receiver_coercion,
            operation,
        } => receiver_coercion.iter().chain([operation]).collect(),
    }
}

#[test]
fn interface_dispatch_enqueues_only_the_selected_interface_implementation_body() {
    let target = build_target_program(&Fixture::with_app(
        "pub interface Readable { pub method &self.read(): i32 }\n\
         struct Value {}\n\
         instance Value {\n\
             impl Readable\n\
             method &self.read(): i32 { 42 }\n\
         }\n\
         func read<T>(value: &T): i32 where T impl Readable { value.read() }\n\
         func main(): i32 {\n\
             let value = Value {}\n\
             read(&value)\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let owners = executable
        .items()
        .iter()
        .filter_map(|(_, item)| match item.key() {
            ExecutableItemKey::Callable(key) => target
                .as_ref()
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
            .any(|owner| matches!(owner, CallableOwner::Instance(_)))
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
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_tests(Arc::clone(&target), selected).unwrap();
    let ExecutableRoot::Tests { cases, .. } = executable.root() else {
        panic!("test selection must produce a test root")
    };
    let graph = target.as_ref().checked().graph();
    assert_eq!(
        cases
            .iter()
            .map(ExecutableTestCase::name)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        cases.iter().all(|case| executable
            .items()
            .get(case.item())
            .is_some_and(|item| matches!(
                executable.types().get(item.signature().result()),
                Some(nocter_model::TypeKind::Fallible(payload))
                    if *payload == executable.types().builtin(BuiltinType::Void)
            )))
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

#[test]
fn executable_type_representations_specialize_complete_nominal_storage() {
    let target = build_target_program(&Fixture::with_app(
        "struct Pair<T> {\n\
             first: T\n\
             second: u8\n\
         }\n\
         enum Choice<T> {\n\
             one(value: T)\n\
             pair(first: T, second: u8)\n\
         }\n\
         func select(value: Choice<i32>): i32 {\n\
             match value {\n\
                 Choice.one(item) { item }\n\
                 Choice.pair(item, _) { item }\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let pair = Pair { first: 1, second: 2 }\n\
             let result = select(Choice.pair(pair.first, pair.second))\n\
             result\n\
         }\n",
    ));
    let target = Arc::new(target);
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = ExecutableProgram::for_executable(Arc::clone(&target), selected).unwrap();
    let graph = target.as_ref().checked().graph();
    let i32_ = executable.types().builtin(BuiltinType::I32);
    let u8_ = executable.types().builtin(BuiltinType::U8);

    let mut pair_seen = false;
    let mut choice_seen = false;
    for (ty, representation) in executable.type_representations().iter() {
        let Some(nocter_model::TypeKind::Nominal { definition, .. }) = executable.types().get(ty)
        else {
            continue;
        };
        let name = graph
            .declarations()
            .nominal_types()
            .get(*definition)
            .and_then(|nominal| graph.symbols().spelling(nominal.name()));
        match (name, representation) {
            (Some("Pair"), RuntimeTypeRepresentation::Struct { fields }) => {
                assert_eq!(
                    fields.iter().map(|field| field.ty()).collect::<Vec<_>>(),
                    [i32_, u8_]
                );
                pair_seen = true;
            }
            (Some("Choice"), RuntimeTypeRepresentation::Enum { variants }) => {
                assert_eq!(variants.len(), 2);
                assert_eq!(
                    variants[0]
                        .payload()
                        .iter()
                        .map(|payload| payload.ty())
                        .collect::<Vec<_>>(),
                    [i32_]
                );
                assert_eq!(
                    variants[1]
                        .payload()
                        .iter()
                        .map(|payload| payload.ty())
                        .collect::<Vec<_>>(),
                    [i32_, u8_]
                );
                choice_seen = true;
            }
            _ => {}
        }
    }
    assert!(pair_seen && choice_seen);
}
