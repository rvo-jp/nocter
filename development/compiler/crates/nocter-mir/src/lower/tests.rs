use nocter_checking::{check_prepared_program, is_concrete_type, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_model::CompilationTarget;
use nocter_target_program::{
    ExecutableItemKey, ExecutableProgram, PrimitiveBinding, PrimitiveRegistry, PrimitiveRole,
    TargetProgram, ToolchainSnapshot,
};
use nocter_test_support::CompilerFixture;

use super::{MirLoweringError, lower_executable};
use crate::{MirAggregate, MirOperationKind, MirProjectionKind, MirTerminator};

mod interpolation;
mod root;

#[test]
fn lowers_scalar_control_flow_through_the_complete_frontend() {
    let program = lower_fixture(
        "func main(): i32 {\n\
             let value = 1\n\
             if true { value + 2 } else { 3 }\n\
         }\n",
    )
    .unwrap();
    let function = program.functions().iter().next().unwrap().1;

    assert_eq!(function.blocks().len(), 4);
    assert!(
        function
            .operations()
            .iter()
            .any(|(_, operation)| matches!(operation.kind(), MirOperationKind::Initialize { .. }))
    );
    assert!(
        function
            .operations()
            .iter()
            .any(|(_, operation)| matches!(operation.kind(), MirOperationKind::Binary { .. }))
    );
    assert!(
        function
            .blocks()
            .iter()
            .any(|(_, block)| matches!(block.terminator(), MirTerminator::Branch { .. }))
    );
}

#[test]
fn lowers_generic_direct_calls_to_dense_executable_items() {
    let program = lower_fixture(
        "func identity<T>(value: T): T { move value }\n\
         func main(): i32 { identity(7) }\n",
    )
    .unwrap();

    assert_eq!(program.functions().len(), 2);
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
        })
    }));
}

#[test]
fn lowers_standard_calls_with_their_frozen_concrete_signatures() {
    let program = lower_fixture_with_uses(
        "use std/ptr.addr\n\
         use std/ptr.from_ref\n\
         func main(): usize {\n\
             let value = 1\n\
             addr(from_ref(&value))\n\
         }\n",
        &[&["ptr"], &["ptr"]],
    )
    .unwrap();
    let function = program.functions().iter().next().unwrap().1;
    let primitive_calls = function
        .operations()
        .iter()
        .filter_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Call(call)
                if matches!(
                    call.target(),
                    crate::MirCallTarget::StandardPrimitive { .. }
                ) =>
            {
                Some(call)
            }
            _ => None,
        })
        .count();

    assert_eq!(primitive_calls, 2);
}

#[test]
fn lowers_pointer_destruction_with_its_frozen_plan() {
    let program = lower_fixture_with_uses(
        "use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         func main(): i32 {\n\
             var value = Resource {}\n\
             let pointer = from_ref_mut(&+value)\n\
             drop_value_at_ptr_for_test(pointer, 0)\n\
             return 0\n\
         }\n",
        &[&["ptr"], &["ptr"]],
    )
    .unwrap();
    let dependency = program
        .functions()
        .iter()
        .flat_map(|(_, function)| function.operations().iter())
        .find_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Call(call) => match call.target() {
                crate::MirCallTarget::StandardPrimitive {
                    role: PrimitiveRole::DropValueAtPointer,
                    dependency,
                    ..
                } => Some(dependency),
                _ => None,
            },
            _ => None,
        })
        .expect("drop-value primitive");
    let crate::MirPrimitiveDependency::Destruction {
        subject,
        plan: Some(plan),
    } = dependency
    else {
        panic!("drop-value primitive must retain MIR destruction")
    };

    assert_eq!(plan.ty(), *subject);
    assert!(matches!(
        plan.kind(),
        crate::MirDestructionKind::Struct {
            drop: Some(_),
            fields,
        } if fields.is_empty()
    ));
}

#[test]
fn lowers_static_string_values_as_readonly_str_borrows() {
    let program = lower_fixture(
        "func main(): i32 {\n\
             let text: &str = \"hello\"\n\
             let _ = text\n\
             0\n\
         }\n",
    )
    .unwrap();
    let function = program.functions().iter().next().unwrap().1;

    assert!(function.operations().iter().any(|(_, operation)| {
        let MirOperationKind::Constant(crate::MirConstant::Text(text)) = operation.kind() else {
            return false;
        };
        let ty = operation
            .result()
            .and_then(|value| function.values().get(value))
            .copied()
            .map(crate::MirValue::ty);
        text.as_ref() == "hello"
            && matches!(
                ty.and_then(|ty| program.executable().types().get(ty)),
                Some(nocter_model::TypeKind::Borrow {
                    capability: nocter_model::BorrowCapability::Readonly,
                    referent,
                }) if *referent == program.executable().types().builtin(nocter_model::BuiltinType::Str)
            )
    }));
}

#[test]
fn lowers_typed_string_literals_to_the_declared_constructor() {
    let program = lower_fixture(
        "struct Text {}\n\
         construct Text {\n\
             pub literal \"\"(text: &str): Self {\n\
                 let _ = text\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): void {\n\
             let _ = Text \"hello\"\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Call(call)
                    if matches!(call.target(), crate::MirCallTarget::Direct(_))
                        && call.allocation() == crate::MirCallAllocation::Inherit
                        && call.arguments().len() == 1
                        && function.values().get(call.arguments()[0]).is_some_and(|value| {
                            matches!(
                                value.definition(),
                                crate::MirValueDefinition::Operation(operation)
                                    if matches!(
                                        function.operations().get(operation).map(crate::MirOperation::kind),
                                        Some(MirOperationKind::Constant(crate::MirConstant::Text(text)))
                                            if text.as_ref() == "hello"
                                    )
                            )
                        })
            )
        })
    }));
}

#[test]
fn lowers_literal_pack_body_through_its_dedicated_input_lane() {
    let executable = executable_fixture(&CompilerFixture::with_app(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 let length = items.len()\n\
                 for item in items {}\n\
                 let _ = length\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let values = Vec [1, 2]\n\
             drop values\n\
             0\n\
         }\n",
    ));
    let (item_id, item) = executable
        .items()
        .iter()
        .find(|(_, item)| item.signature().pack().is_some())
        .unwrap();
    let function = super::function::lower_function(&executable, item_id, item).unwrap();
    let pack = item.signature().pack().unwrap();

    assert_eq!(
        function.pack(),
        Some(crate::MirPackInput::new(pack.element(), pack.next()))
    );
    assert!(
        function
            .operations()
            .iter()
            .any(|(_, operation)| { matches!(operation.kind(), MirOperationKind::PackLength) })
    );
    assert!(
        function
            .operations()
            .iter()
            .any(|(_, operation)| { matches!(operation.kind(), MirOperationKind::PackNext) })
    );
    assert!(function.blocks().iter().any(|(_, block)| {
        matches!(
            block.terminator(),
            MirTerminator::Switch { cases, .. }
                if cases.iter().any(|case| case.value() == crate::MirSwitchValue::OptionalPresent)
        )
    }));
    for (_, block) in function
        .blocks()
        .iter()
        .filter(|(_, block)| matches!(block.terminator(), MirTerminator::Return(_)))
    {
        let last = block.operations().last().copied().unwrap();
        assert!(matches!(
            function
                .operations()
                .get(last)
                .map(crate::MirOperation::kind),
            Some(MirOperationKind::DestroyPack)
        ));
    }
}

#[test]
fn lowers_sequence_calls_through_the_hidden_pack_lane() {
    let program = lower_fixture(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let _ = Vec [1, 2]\n\
             0\n\
         }\n",
    )
    .unwrap();
    let (constructor, input) = program
        .functions()
        .iter()
        .find_map(|(item, function)| function.pack().map(|input| (item, input)))
        .unwrap();
    let (main_function, pack) = program
        .functions()
        .iter()
        .find_map(|(_, function)| {
            function.operations().iter().find_map(|(_, operation)| {
                let MirOperationKind::Call(call) = operation.kind() else {
                    return None;
                };
                matches!(call.target(), crate::MirCallTarget::Direct(item) if *item == constructor)
                    .then_some(())
                    .and_then(|()| call.pack().map(|pack| (function, pack)))
            })
        })
        .unwrap();

    assert!(pack.segments().iter().all(|segment| {
        matches!(
            segment,
            crate::MirPackSegment::Value {
                destruction: None,
                ..
            }
        )
    }));
    assert_eq!(pack.segments().len(), 2);
    assert_eq!(
        (pack.element(), pack.next()),
        (input.element(), input.next())
    );
    assert!(
        main_function
            .values()
            .get(pack.length())
            .is_some_and(|value| {
                matches!(
                    value.definition(),
                    crate::MirValueDefinition::Operation(operation)
                        if matches!(
                            main_function
                                .operations()
                                .get(operation)
                                .map(crate::MirOperation::kind),
                            Some(MirOperationKind::Constant(crate::MirConstant::Integer(2)))
                        )
                )
            })
    );
}

#[test]
fn lowers_exact_size_spreads_with_deferred_residual_destruction() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 let _ = items.len()\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         struct Leaf {}\n\
         drop Leaf(&+self) { return }\n\
         struct Item { leaf: Leaf }\n\
         struct Iter {}\n\
         conform Iterator for Iter {\n\
             type Item = Item\n\
             method &+self.next(): Item? { return none }\n\
         }\n\
         conform ExactSizeIterator for Iter {\n\
             method &self.remaining_len(): usize { return 0 }\n\
         }\n\
         drop Iter(&+self) { return }\n\
         func main(): i32 {\n\
             let iterator = Iter {}\n\
             let _ = Vec [Item { leaf: Leaf {} }, ...move iterator, Item { leaf: Leaf {} }]\n\
             0\n\
         }\n",
        &[&[], &[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();
    let (caller, pack) = program
        .functions()
        .iter()
        .find_map(|(_, function)| {
            function.operations().iter().find_map(|(_, operation)| {
                let MirOperationKind::Call(call) = operation.kind() else {
                    return None;
                };
                call.pack().map(|pack| (function, pack))
            })
        })
        .unwrap();
    let [
        crate::MirPackSegment::Value {
            destruction: Some(first),
            ..
        },
        crate::MirPackSegment::Spread(spread),
        crate::MirPackSegment::Value {
            destruction: Some(last),
            ..
        },
    ] = pack.segments()
    else {
        panic!("expected fixed, spread, fixed pack order")
    };

    assert_eq!(first.ty(), pack.element());
    assert_eq!(last.ty(), pack.element());
    assert!(matches!(
        first.kind(),
        crate::MirDestructionKind::Struct { fields, .. }
            if matches!(fields.as_ref(), [field]
                if matches!(
                    field.plan().kind(),
                    crate::MirDestructionKind::Struct { drop: Some(_), .. }
                ))
    ));
    assert!(spread.destruction().is_some());
    assert_eq!(spread.contribution(), crate::MirPackContribution::Direct);
    assert!(matches!(
        caller
            .values()
            .get(spread.remaining())
            .copied()
            .map(crate::MirValue::definition),
        Some(crate::MirValueDefinition::Operation(operation))
            if matches!(
                caller.operations().get(operation).map(crate::MirOperation::kind),
                Some(MirOperationKind::Call(call)) if call.pack().is_none()
            )
    ));
    assert!(caller.operations().iter().any(|(_, operation)| {
        matches!(
            operation.kind(),
            MirOperationKind::Binary {
                operation: crate::MirBinaryOperation::Add,
                ..
            }
        )
    }));
}

#[test]
fn sequence_using_retains_the_same_call_scoped_allocation_lane() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let allocator = Allocator { state: 0, kind: 0 }\n\
             let _ = Vec [1] using allocator\n\
             0\n\
         }\n",
        &[&[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            let MirOperationKind::Call(call) = operation.kind() else {
                return false;
            };
            let crate::MirCallAllocation::Explicit(place) = call.allocation() else {
                return false;
            };
            call.pack().is_some()
                && function.places().get(place).is_some_and(|place| {
                    matches!(
                        program.executable().types().get(place.ty()),
                        Some(nocter_model::TypeKind::Nominal { definition, .. })
                            if Some(*definition)
                                == program
                                    .executable()
                                    .target()
                                    .checked()
                                    .standard_semantics()
                                    .nominal(nocter_declarations::StandardDeclarationRole::AbortingAllocator)
                    )
                })
        })
    }));
}

#[test]
fn lowers_opaque_results_with_their_specialized_witness() {
    let program = lower_fixture(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Value {}\n\
         conform Show for Value { method &self.show(): i32 { 7 } }\n\
         func make(): some Show { Value {} }\n\
         func main(): i32 { make().show() }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Aggregate(MirAggregate::Opaque { .. })
            )
        })
    }));
    assert!(program.functions().iter().any(|(_, function)| {
        function.places().iter().any(|(_, place)| {
            place
                .projections()
                .iter()
                .any(|projection| matches!(projection.kind(), MirProjectionKind::OpaqueWitness(_)))
        })
    }));
}

#[test]
fn opens_owned_and_readwrite_opaque_receivers_without_erasing_capability() {
    let program = lower_fixture(
        "pub interface Consume { pub method self.consume(): i32 }\n\
         pub interface Mutate { pub method &+self.bump(): i32 }\n\
         struct OwnedValue {}\n\
         struct MutableValue { value: i32 }\n\
         conform Consume for OwnedValue { method self.consume(): i32 { 2 } }\n\
         conform Mutate for MutableValue {\n\
             method &+self.bump(): i32 {\n\
                 self.value += 1\n\
                 self.value\n\
             }\n\
         }\n\
         func make_owned(): some Consume { OwnedValue {} }\n\
         func make_mutable(): some Mutate { MutableValue { value: 3 } }\n\
         func main(): i32 {\n\
             let consumed = make_owned().consume()\n\
             var mutable = make_mutable()\n\
             consumed + mutable.bump()\n\
         }\n",
    )
    .unwrap();

    let opaque_projections = program
        .functions()
        .iter()
        .flat_map(|(_, function)| function.places().iter())
        .flat_map(|(_, place)| place.projections())
        .filter(|projection| matches!(projection.kind(), MirProjectionKind::OpaqueWitness(_)))
        .count();
    assert!(opaque_projections >= 2);
}

#[test]
fn typed_string_using_retains_an_explicit_call_scoped_allocation_place() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         struct Text {}\n\
         construct Text {\n\
             pub literal \"\"(text: &str): Self { return Self {} }\n\
         }\n\
         func main(): void {\n\
             let allocator = Allocator { state: 0, kind: 0 }\n\
             let _ = Text \"hello\" using allocator\n\
             return\n\
         }\n",
        &[&[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            let MirOperationKind::Call(call) = operation.kind() else {
                return false;
            };
            let crate::MirCallAllocation::Explicit(place) = call.allocation() else {
                return false;
            };
            matches!(call.target(), crate::MirCallTarget::Direct(_))
                && function.places().get(place).is_some_and(|place| {
                    matches!(
                        program.executable().types().get(place.ty()),
                        Some(nocter_model::TypeKind::Nominal { definition, arguments })
                            if arguments.is_empty()
                                && Some(*definition)
                                    == program
                                        .executable()
                                        .target()
                                        .checked()
                                        .standard_semantics()
                                        .nominal(nocter_declarations::StandardDeclarationRole::AbortingAllocator)
                    )
                })
        })
    }));
}

#[test]
fn lowers_primitive_comparison_from_frozen_prepared_borrows() {
    let program = lower_fixture("func main(): i32 { if 1 == 2 { 0 } else { 1 } }\n").unwrap();
    let function = program.functions().iter().next().unwrap().1;

    assert_eq!(
        function
            .operations()
            .iter()
            .filter(|(_, operation)| matches!(operation.kind(), MirOperationKind::Borrow { .. }))
            .count(),
        2
    );
    assert!(function.operations().iter().any(|(_, operation)| matches!(
        operation.kind(),
        MirOperationKind::Call(call)
            if matches!(call.target(), crate::MirCallTarget::Structural(crate::MirStructuralCall::Equality { .. }))
    )));
}

#[test]
fn lowers_inherent_method_receivers_without_reopening_selection() {
    let fixture = CompilerFixture::with_app(
        "copy struct Value { field: i32 }\n\
         instance Value {\n\
             pub method &self.read(): i32 { self.field }\n\
         }\n\
         func main(): i32 {\n\
             let input = Value { field: 7 }\n\
             input.read()\n\
         }\n",
    );
    let executable = executable_fixture(&fixture);
    let program = lower_executable(executable).unwrap();

    assert_eq!(program.functions().len(), 2);
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)) && call.arguments().len() == 1)
        })
    }));
}

#[test]
fn lowers_selected_comparison_with_source_order_operand_preparation() {
    let program = lower_fixture(
        "copy struct Value { field: i32 }\n\
         instance Value {\n\
             pub operator (&self == another: &Self): bool {\n\
                 self.field == another.field\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let left = Value { field: 1 }\n\
             let right = Value { field: 2 }\n\
             if left == right { 0 } else { 1 }\n\
         }\n",
    )
    .unwrap();

    assert_eq!(program.functions().len(), 2);
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)) && call.arguments().len() == 2)
        })
    }));
}

#[test]
fn lowers_checked_comparison_operand_coercions_before_the_operation() {
    let program = lower_fixture(
        "copy struct Text { value: i32 }\n\
         copy struct Wrapper { text: Text }\n\
         instance Text {\n\
             pub operator (&self == another: &Self): bool {\n\
                 self.value == another.value\n\
             }\n\
         }\n\
         instance Wrapper { pub coerce &self as &Text { &self.text } }\n\
         func main(): i32 {\n\
             let left = Text { value: 1 }\n\
             let right = Wrapper { text: Text { value: 2 } }\n\
             if left == right { 0 } else { 1 }\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
            })
            .count()
            == 2
    }));
}

#[test]
fn lowers_specialized_generic_comparison_with_operand_lanes_intact() {
    let program = lower_fixture(
        "copy struct Text { value: i32 }\n\
         copy struct Wrapper { text: Text }\n\
         instance Text {\n\
             pub operator (&self == another: &Self): bool {\n\
                 self.value == another.value\n\
             }\n\
         }\n\
         instance Wrapper { pub coerce &self as &Text { &self.text } }\n\
         func same<T>(left: &T, right: &T): bool where (&T == &T): bool {\n\
             left == right\n\
         }\n\
         func main(): i32 {\n\
             let left = Wrapper { text: Text { value: 1 } }\n\
             let right = Wrapper { text: Text { value: 2 } }\n\
             if same(&left, &right) { 0 } else { 1 }\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
            })
            .count()
            == 3
    }));
}

#[test]
fn lowers_selected_borrow_conversion_as_an_ordinary_call() {
    let program = lower_fixture(
        "copy struct Text { value: i32 }\n\
         copy struct Wrapper { text: Text }\n\
         instance Wrapper {\n\
             pub coerce &self as &Text { &self.text }\n\
         }\n\
         func accept(value: &Text): i32 { value.value }\n\
         func main(): i32 {\n\
             let wrapper = Wrapper { text: Text { value: 7 } }\n\
             accept(&wrapper)\n\
         }\n",
    )
    .unwrap();

    assert_eq!(program.functions().len(), 3);
    assert!(program.functions().iter().any(|(_, function)| {
        function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
            })
            .count()
            == 2
    }));
}

#[test]
fn lowers_method_receiver_coercion_before_the_selected_method() {
    let program = lower_fixture(
        "copy struct Text { value: i32 }\n\
         copy struct Wrapper { text: Text }\n\
         instance Text { pub method &self.len(): usize { 1 } }\n\
         instance Wrapper { pub coerce &self as &Text { &self.text } }\n\
         func main(): usize {\n\
             let wrapper = Wrapper { text: Text { value: 7 } }\n\
             wrapper.len()\n\
         }\n",
    )
    .unwrap();

    assert_eq!(program.functions().len(), 3);
    assert!(program.functions().iter().any(|(_, function)| {
        function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
            })
            .count()
            == 2
    }));
}

#[test]
fn lowers_selected_index_results_as_borrow_rooted_places() {
    let program = lower_fixture(
        "copy struct Buffer { values: [i32; 2] }\n\
         instance Buffer {\n\
             pub operator (&self[index: usize]): &i32 {\n\
                 &self.values[index]\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let buffer = Buffer { values: [1, 2] }\n\
             buffer[1]\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .places()
            .iter()
            .any(|(_, place)| matches!(place.root(), crate::MirPlaceRoot::Dereference { .. }))
    }));
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::Call(call) if matches!(call.target(), crate::MirCallTarget::Direct(_)))
        })
    }));
}

#[test]
fn continues_field_projection_after_a_readwrite_index_call() {
    let program = lower_fixture(
        "copy struct Element { value: i32 }\n\
         copy struct Buffer { values: [Element; 1] }\n\
         instance Buffer {\n\
             pub operator (&self[index: usize]): &Element {\n\
                 &self.values[index]\n\
             }\n\
             pub operator (&+self[index: usize]): &+Element {\n\
                 &+self.values[index]\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             var buffer = Buffer { values: [Element { value: 1 }] }\n\
             buffer[0].value = 7\n\
             buffer[0].value\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::Store { destination, .. }
                if matches!(function.places().get(*destination).unwrap().root(), crate::MirPlaceRoot::Dereference { .. })
                    && !function.places().get(*destination).unwrap().projections().is_empty())
        })
    }));
}

#[test]
fn lowers_coerced_builtin_index_without_reopening_index_selection() {
    let program = lower_fixture(
        "copy struct Wrapper { values: [i32; 2] }\n\
         instance Wrapper {\n\
             pub coerce &self as &[i32; 2] { &self.values }\n\
         }\n\
         func main(): i32 {\n\
             let wrapper = Wrapper { values: [1, 2] }\n\
             wrapper[1]\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .places()
            .iter()
            .any(|(_, place)| matches!(place.root(), crate::MirPlaceRoot::Dereference { .. }))
    }));
}

#[test]
fn lowers_specialized_structural_index_with_its_receiver_lane() {
    let program = lower_fixture(
        "copy struct Buffer { values: [i32; 2] }\n\
         instance Buffer {\n\
             pub operator (&self[index: usize]): &i32 {\n\
                 &self.values[index]\n\
             }\n\
         }\n\
         func read<C, V>(source: &C, index: usize): V where copy V, (&C[usize]): &V {\n\
             source[index]\n\
         }\n\
         func main(): i32 {\n\
             let buffer = Buffer { values: [1, 2] }\n\
             read(&buffer, 1)\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .places()
            .iter()
            .any(|(_, place)| matches!(place.root(), crate::MirPlaceRoot::Dereference { .. }))
    }));
}

#[test]
fn lowers_fallible_injection_and_propagation_through_typed_storage() {
    let program = lower_fixture(
        "func pass(input: i32!): i32! { input? }\n\
         func main(): i32! { pass(1) }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .blocks()
            .iter()
            .any(|(_, block)| matches!(block.terminator(), MirTerminator::Switch { .. }))
    }));
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Aggregate(crate::MirAggregate::FallibleFailure(_))
            )
        })
    }));
}

#[test]
fn lowers_forced_optional_failure_to_an_explicit_trap_edge() {
    let program = lower_fixture(
        "func force(input: i32?): i32 { input! }\n\
         func main(): i32 { force(1) }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .blocks()
            .iter()
            .any(|(_, block)| matches!(block.terminator(), MirTerminator::Trap))
    }));
}

#[test]
fn lowers_fallible_recovery_binding_and_value_join() {
    let program = lower_fixture(
        "func recover(input: i32!): i32 { input catch failure { 0 } }\n\
         func main(): i32 { recover(1) }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .blocks()
            .iter()
            .any(|(_, block)| !block.parameters().is_empty())
    }));
}

#[test]
fn preserves_outer_outcome_layers_on_propagation_failure_edges() {
    let program = lower_fixture(
        "func lift_absence(input: i32?): (i32?)! { input? }\n\
         func lift_failure(input: i32!): (i32!)? { input? }\n\
         func main(): i32 {\n\
             let _ = lift_absence(none)\n\
             let _ = lift_failure(1)\n\
             0\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Aggregate(crate::MirAggregate::FallibleSuccess(_))
            )
        })
    }));
    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Aggregate(crate::MirAggregate::Optional(Some(_)))
            )
        })
    }));
}

#[test]
fn lowers_checked_user_destruction_once_without_recursing_on_its_receiver() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func main(): void {\n\
             let value = Owned { value: 1 }\n\
             return\n\
         }\n",
    )
    .unwrap();

    let invocations = program
        .functions()
        .iter()
        .flat_map(|(_, function)| function.operations().iter())
        .filter(|(_, operation)| matches!(operation.kind(), MirOperationKind::InvokeDrop { .. }))
        .count();

    assert_eq!(invocations, 1);
}

#[test]
fn conditional_path_cleanup_uses_one_flag_across_move_and_reinitialization() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func restore(condition: bool, first: Owned, second: Owned): void {\n\
             var value = move first\n\
             if condition { let _ = move value }\n\
             value = move second\n\
             return\n\
         }\n\
         func main(): void {\n\
             restore(true, Owned { value: 1 }, Owned { value: 2 })\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.drop_flags().len() == 1
            && function.blocks().iter().any(|(_, block)| {
                matches!(
                    block.terminator(),
                    crate::MirTerminator::BranchDropFlag { .. }
                )
            })
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::SetDropFlag {
                        initialized: false,
                        ..
                    }
                )
            })
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::SetDropFlag {
                        initialized: true,
                        ..
                    }
                )
            })
    }));
}

#[test]
fn branch_only_owned_temporaries_use_entry_initialized_drop_flags() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         instance Owned {\n\
             pub operator (&self == other: &Self): bool { self.value == other.value }\n\
         }\n\
         func main(): void {\n\
             let _ = if true {\n\
                 Owned { value: 1 } == Owned { value: 2 }\n\
             } else {\n\
                 true\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.drop_flags().len() == 2
            && function
                .drop_flags()
                .iter()
                .all(|(_, flag)| !flag.initially_initialized())
            && function.blocks().iter().any(|(_, block)| {
                matches!(
                    block.terminator(),
                    crate::MirTerminator::BranchDropFlag { .. }
                )
            })
    }));
}

#[test]
fn borrowed_temporary_and_cleanup_share_one_storage_slot() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         instance Owned {\n\
             pub operator (&self == other: &Self): bool { self.value == other.value }\n\
         }\n\
         func main(): void {\n\
             let _ = Owned { value: 1 } == Owned { value: 2 }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        let initialized = function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::Initialize { .. })
            })
            .count();
        let dropped = function
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::InvokeDrop { .. })
            })
            .count();
        initialized == 2 && dropped == 2
    }));
}

#[test]
fn lowers_while_infinite_and_range_loops_with_explicit_transfer_targets() {
    let program = lower_fixture(
        "func main(): void {\n\
             var value = 0\n\
             value += 2\n\
             while false { continue }\n\
             loop { break }\n\
             for index in 0..<3 { let _ = index }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Binary {
                    operation: crate::MirBinaryOperation::Add,
                    ..
                }
            )
        }) && function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Binary {
                    operation: crate::MirBinaryOperation::Less,
                    ..
                }
            )
        }) && function.blocks().len() >= 10
    }));
}

#[test]
fn never_loop_closes_without_an_invented_exit_block() {
    let program = lower_fixture("func main(): void { loop {} }\n").unwrap();
    let (_, function) = program.functions().iter().next().unwrap();

    assert_eq!(function.blocks().len(), 2);
    assert!(
        function
            .blocks()
            .iter()
            .all(|(_, block)| { matches!(block.terminator(), crate::MirTerminator::Goto(_)) })
    );
}

#[test]
fn explicit_drop_and_fallthrough_blocks_use_the_same_cleanup_lowering() {
    for main in [
        "func main(): void {\n    let value = Owned { value: 1 }\n    drop value\n    return\n}",
        "func main(): void {\n    let value = Owned { value: 1 }\n}",
    ] {
        let program = lower_fixture(&format!(
            "struct Owned {{ value: i32 }}\n\
             drop Owned(&+self) {{ return }}\n\
             {main}\n"
        ))
        .unwrap();
        let invocations = program
            .functions()
            .iter()
            .flat_map(|(_, function)| function.operations().iter())
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::InvokeDrop { .. })
            })
            .count();

        assert_eq!(invocations, 1);
    }
}

#[test]
fn lowers_collection_iteration_from_frozen_acquisition_and_next_dispatch() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         struct Source { remaining: i32 }\n\
         struct Iter { remaining: i32 }\n\
         drop Iter(&+self) { return }\n\
         instance Source {\n\
             pub operator (...self): Iter {\n\
                 return Iter { remaining: self.remaining }\n\
             }\n\
         }\n\
         conform Iterator for Iter {\n\
             type Item = i32\n\
             method &+self.next(): i32? {\n\
                 if self.remaining == 0 {\n\
                     return none\n\
                 }\n\
                 self.remaining -= 1\n\
                 return self.remaining\n\
             }\n\
         }\n\
         func main(): void {\n\
             let source = Source { remaining: 2 }\n\
             for item in move source {\n\
                 if item == 0 {\n\
                     continue\n\
                 }\n\
                 break\n\
             }\n\
             return\n\
         }\n",
        &[&[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.drop_flags().len() == 1 && function.blocks().iter().any(|(_, block)| {
            matches!(
                block.terminator(),
                crate::MirTerminator::Switch {
                    subject: crate::MirSwitchSubject::Place(_),
                    cases,
                    ..
                } if cases.iter().any(|case| case.value() == crate::MirSwitchValue::OptionalPresent)
            )
        }) && function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Borrow {
                    capability: nocter_model::BorrowCapability::ReadWrite,
                    ..
                }
            )
        })
    }));
}

#[test]
fn collection_iteration_opens_an_opaque_iterator_receiver() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         struct Iter { remaining: i32 }\n\
         conform Iterator for Iter {\n\
             type Item = i32\n\
             method &+self.next(): i32? {\n\
                 if self.remaining == 0 {\n\
                     return none\n\
                 }\n\
                 self.remaining -= 1\n\
                 return self.remaining\n\
             }\n\
         }\n\
         func make(): some Iterator<Item = i32> { Iter { remaining: 2 } }\n\
         func main(): void {\n\
             var iterator = make()\n\
             for item in move iterator {\n\
                 let _ = item\n\
             }\n\
             return\n\
         }\n",
        &[&[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        let opens_witness = function.places().iter().any(|(_, place)| {
            place
                .projections()
                .iter()
                .any(|projection| matches!(projection.kind(), MirProjectionKind::OpaqueWitness(_)))
        });
        let iterates = function.blocks().iter().any(|(_, block)| {
            matches!(
                block.terminator(),
                MirTerminator::Switch { cases, .. }
                    if cases.iter().any(|case| case.value() == crate::MirSwitchValue::OptionalPresent)
            )
        });
        opens_witness && iterates
    }));
}

#[test]
fn lowers_generic_enum_patterns_with_specialized_payload_types() {
    let program = lower_fixture(
        "enum Choice<T> {\n    some(value: T)\n    empty\n}\n\
         func select(value: Choice<i32>): i32 {\n\
             match value {\n\
                 Choice.some(item) { item }\n\
                 Choice.empty { 0 }\n\
             }\n\
         }\n\
         func main(): i32 { select(Choice.some(7)) }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.blocks().iter().any(|(_, block)| {
            matches!(
                block.terminator(),
                MirTerminator::Switch { cases, .. } if cases.len() == 2
            )
        }) && function.operations().iter().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MirOperationKind::Read {
                    mode: crate::MirReadMode::Copy,
                    ..
                }
            )
        }) && function
            .blocks()
            .iter()
            .any(|(_, block)| !block.parameters().is_empty())
    }));
}

#[test]
fn lowers_borrowed_pattern_payloads_without_owning_the_enum() {
    let program = lower_fixture(
        "enum Slot { value(item: i32) }\n\
         func inspect(input: &+Slot): void {\n\
             match input {\n\
                 Slot.value(item) {}\n\
             }\n\
             return\n\
         }\n\
         func main(): void {\n\
             var slot = Slot.value(7)\n\
             inspect(&+slot)\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function
            .places()
            .iter()
            .any(|(_, place)| matches!(place.root(), crate::MirPlaceRoot::Dereference { .. }))
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Borrow {
                        capability: nocter_model::BorrowCapability::ReadWrite,
                        ..
                    }
                )
            })
            && function.drop_flags().is_empty()
    }));
}

#[test]
fn invokes_enum_drop_before_moving_payload_and_cleans_only_the_residual() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         enum Resource { active(item: Owned, retained: Owned) }\n\
         drop Resource(&+self) { return }\n\
         func main(): void {\n\
             match Resource.active(Owned { value: 1 }, Owned { value: 2 }) {\n\
                 Resource.active(item, _) { let _ = move item }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        let operations = function
            .operations()
            .iter()
            .map(|(_, operation)| operation.kind())
            .collect::<Vec<_>>();
        let owner_drop = operations
            .iter()
            .position(|operation| matches!(operation, MirOperationKind::InvokeDrop { .. }));
        let payload_move = operations.iter().position(|operation| {
            matches!(
                operation,
                MirOperationKind::Read {
                    mode: crate::MirReadMode::Move,
                    ..
                }
            )
        });
        owner_drop
            .zip(payload_move)
            .is_some_and(|(drop, move_)| drop < move_)
            && operations
                .iter()
                .filter(|operation| matches!(operation, MirOperationKind::InvokeDrop { .. }))
                .count()
                == 3
    }));
}

#[test]
fn pattern_match_and_unmatched_cleanup_use_distinct_flags_on_shared_storage() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         func main(): void {\n\
             if Pair.values(Owned { value: 1 }, Owned { value: 2 }) is Pair.values(item, _) {\n\
                 let _ = move item\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        let flagged_places = function
            .drop_flags()
            .iter()
            .map(|(_, flag)| flag.place())
            .collect::<Vec<_>>();
        flagged_places.len() == 2
            && flagged_places[0] == flagged_places[1]
            && function.blocks().iter().any(|(_, block)| {
                matches!(block.terminator(), MirTerminator::BranchDropFlag { .. })
            })
    }));
}

#[test]
fn lowers_nested_regions_with_ordered_early_exit_cleanup() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func normal(allocator: &Allocator): void {\n\
             region temp using allocator {\n\
                 let value = Owned { value: 0 }\n\
             }\n\
             return\n\
         }\n\
         func main(): void {\n\
             let allocator = Allocator { state: 0, kind: 0 }\n\
             normal(&allocator)\n\
             region outer using allocator {\n\
                 let first = Owned { value: 1 }\n\
                 region inner using outer {\n\
                     let second = Owned { value: 2 }\n\
                     return\n\
                 }\n\
             }\n\
         }\n",
        &[&[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        let lifetime_operations = function
            .operations()
            .iter()
            .filter_map(|(_, operation)| match operation.kind() {
                MirOperationKind::CreateRegion { .. } => Some("create"),
                MirOperationKind::InvokeDrop { .. } => Some("drop"),
                MirOperationKind::ReleaseRegion { .. } => Some("release"),
                _ => None,
            })
            .collect::<Vec<_>>();
        lifetime_operations == ["create", "create", "drop", "release", "drop", "release"]
    }));
    assert!(program.functions().iter().any(|(_, function)| {
        function
            .operations()
            .iter()
            .filter_map(|(_, operation)| match operation.kind() {
                MirOperationKind::CreateRegion { .. } => Some("create"),
                MirOperationKind::InvokeDrop { .. } => Some("drop"),
                MirOperationKind::ReleaseRegion { .. } => Some("release"),
                _ => None,
            })
            .eq(["create", "drop", "release"])
    }));
}

#[test]
fn lowers_closure_construction_and_invocation_from_one_concrete_layout() {
    let program = lower_fixture(
        "func main(): i32 {\n\
             let double = (value: i32): i32 { value * 2 }\n\
             double(3)\n\
         }\n",
    )
    .unwrap();

    assert_eq!(program.functions().len(), 2);
    let closure_items = program
        .executable()
        .items()
        .iter()
        .filter(|(_, item)| matches!(item.key(), ExecutableItemKey::Closure(_)))
        .collect::<Vec<_>>();
    assert_eq!(closure_items.len(), 1);
    let (closure_item, item) = closure_items[0];
    let layout = item.closure_layout().unwrap();
    assert!(layout.captures().is_empty());
    assert!(program.functions().iter().any(|(_, function)| {
        let mut constructed = false;
        let mut invoked = false;
        for (_, operation) in function.operations().iter() {
            match operation.kind() {
                MirOperationKind::Aggregate(MirAggregate::Closure { body, captures }) => {
                    constructed |= *body == closure_item && captures.is_empty();
                }
                MirOperationKind::Call(call) => {
                    invoked |= matches!(
                        call.target(),
                        crate::MirCallTarget::Direct(body) if *body == closure_item
                    ) && call.arguments().len() == 2;
                }
                _ => {}
            }
        }
        constructed && invoked
    }));
}

#[test]
fn lowers_readonly_capture_through_the_frozen_environment_representation() {
    let program = lower_fixture(
        "func main(): i32 {\n\
             let value = 7\n\
             let read = (&value;): i32 { value }\n\
             read()\n\
         }\n",
    )
    .unwrap();
    let closure = only_closure_function(&program);

    assert!(closure.places().iter().any(|(_, place)| {
        matches!(
            place.projections(),
            [
                first,
                capture,
                stored
            ] if first.kind() == MirProjectionKind::BorrowDereference(nocter_model::BorrowCapability::Readonly)
                && matches!(capture.kind(), MirProjectionKind::ClosureCapture(_))
                && stored.kind() == MirProjectionKind::BorrowDereference(nocter_model::BorrowCapability::Readonly)
        )
    }));
}

#[test]
fn lowers_readwrite_capture_without_losing_the_outer_authority_ceiling() {
    let program = lower_fixture(
        "func main(): i32 {\n\
             var total = 0\n\
             var increment = (&+total;): i32 {\n\
                 total += 1\n\
                 total\n\
             }\n\
             increment()\n\
         }\n",
    )
    .unwrap();
    let closure = only_closure_function(&program);

    assert!(
        closure
            .operations()
            .iter()
            .any(|(_, operation)| { matches!(operation.kind(), MirOperationKind::Store { .. }) })
    );
    assert!(closure.places().iter().any(|(_, place)| {
        matches!(
            place.projections(),
            [
                first,
                capture,
                stored
            ] if first.kind() == MirProjectionKind::BorrowDereference(nocter_model::BorrowCapability::ReadWrite)
                && matches!(capture.kind(), MirProjectionKind::ClosureCapture(_))
                && stored.kind() == MirProjectionKind::BorrowDereference(nocter_model::BorrowCapability::ReadWrite)
        )
    }));
}

#[test]
fn lowers_owned_capture_consumption_and_closure_destruction_exactly_once() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func consume(value: Owned): i32 { value.value }\n\
         func main(): i32 {\n\
             let value = Owned { value: 7 }\n\
             let take = (move value;): i32 { consume(move value) }\n\
             take()\n\
         }\n",
    )
    .unwrap();

    let closure = only_closure_function(&program);
    assert!(closure.operations().iter().any(|(_, operation)| {
        let MirOperationKind::Read {
            place,
            mode: crate::MirReadMode::Move,
        } = operation.kind()
        else {
            return false;
        };
        closure
            .places()
            .get(*place)
            .is_some_and(place_has_capture_projection)
    }));

    assert!(!closure.operations().iter().any(|(_, operation)| {
        let MirOperationKind::InvokeDrop { place, .. } = operation.kind() else {
            return false;
        };
        closure
            .places()
            .get(*place)
            .is_some_and(place_has_capture_projection)
    }));
}

#[test]
fn lowers_unused_owned_capture_through_closure_destruction() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func main(): void {\n\
             let value = Owned { value: 7 }\n\
             let keep = (move value;): void { return }\n\
             return\n\
         }\n",
    )
    .unwrap();

    assert!(program.functions().iter().any(|(_, function)| {
        function.operations().iter().any(|(_, operation)| {
            let MirOperationKind::InvokeDrop { place, .. } = operation.kind() else {
                return false;
            };
            function
                .places()
                .get(*place)
                .is_some_and(place_has_capture_projection)
        })
    }));
}

#[test]
fn specializes_generic_closure_environment_before_mir_lowering() {
    let program = lower_fixture(
        "func inspect<T>(value: &T): void {\n\
             let closure = (&value;): void {\n\
                 let _ = value\n\
                 return\n\
             }\n\
             closure()\n\
             return\n\
         }\n\
         func main(): void {\n\
             let value = 7\n\
             inspect(&value)\n\
             return\n\
         }\n",
    )
    .unwrap();
    let layouts = program
        .executable()
        .items()
        .iter()
        .filter_map(|(_, item)| item.closure_layout())
        .collect::<Vec<_>>();

    assert_eq!(layouts.len(), 1);
    assert!(is_concrete_type(program.executable().types(), layouts[0].ty()).unwrap());
    assert!(
        layouts[0].captures().iter().all(|capture| {
            is_concrete_type(program.executable().types(), capture.ty()).unwrap()
        })
    );
}

#[test]
fn conditionally_consumed_capture_starts_initialized_and_drops_only_on_the_residual_path() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func consume(value: Owned): void {\n\
             let _ = value.value\n\
             return\n\
         }\n\
         func main(): void {\n\
             let value = Owned { value: 7 }\n\
             let take = (move value; flag: bool): void {\n\
                 if flag { consume(move value) }\n\
                 return\n\
             }\n\
             take(false)\n\
             return\n\
         }\n",
    )
    .unwrap();
    let closure = only_closure_function(&program);

    assert!(closure.drop_flags().iter().any(|(_, flag)| {
        flag.initially_initialized()
            && closure
                .places()
                .get(flag.place())
                .is_some_and(place_has_capture_projection)
    }));
    assert!(
        closure.blocks().iter().any(|(_, block)| {
            matches!(block.terminator(), MirTerminator::BranchDropFlag { .. })
        })
    );
    assert!(closure.operations().iter().any(|(_, operation)| {
        let MirOperationKind::InvokeDrop { place, .. } = operation.kind() else {
            return false;
        };
        closure
            .places()
            .get(*place)
            .is_some_and(place_has_capture_projection)
    }));
}

#[test]
fn specializes_callable_bounds_to_direct_closure_calls() {
    let program = lower_fixture(
        "func apply<F>(callback: F, value: i32): i32 where F: &func(value: i32): i32 {\n\
             callback(value)\n\
         }\n\
         func main(): i32 { apply((value) { value * 2 }, 7) }\n",
    )
    .unwrap();
    let closure_item = program
        .executable()
        .items()
        .iter()
        .find_map(|(item, definition)| {
            matches!(definition.key(), ExecutableItemKey::Closure(_)).then_some(item)
        })
        .unwrap();

    assert!(program.functions().iter().any(|(item, function)| {
        item != closure_item
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Call(call)
                        if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
                            && call.arguments().len() == 2
                )
            })
    }));
}

#[test]
fn specializes_readwrite_callable_bounds_without_erasing_environment_access() {
    let program = lower_fixture(
        "func apply<F>(callback: F): i32 where F: &+func(): i32 {\n\
             var callable = move callback\n\
             callable()\n\
         }\n\
         func main(): i32 {\n\
             var total = 0\n\
             apply((&+total;): i32 {\n\
                 total += 1\n\
                 total\n\
             })\n\
         }\n",
    )
    .unwrap();
    let closure_item = program
        .executable()
        .items()
        .iter()
        .find_map(|(item, definition)| {
            matches!(definition.key(), ExecutableItemKey::Closure(_)).then_some(item)
        })
        .unwrap();

    assert!(program.functions().iter().any(|(item, function)| {
        item != closure_item
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Borrow {
                        capability: nocter_model::BorrowCapability::ReadWrite,
                        ..
                    }
                )
            })
            && function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Call(call)
                        if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
                )
            })
    }));
}

#[test]
fn owned_callable_contract_destroys_a_readonly_body_environment_after_return() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func finish<F>(callback: F): i32 where F: func(): i32 { callback() }\n\
         func main(): i32 {\n\
             let value = Owned { value: 7 }\n\
             finish((move value;): i32 { value.value })\n\
         }\n",
    )
    .unwrap();
    let closure_item = program
        .executable()
        .items()
        .iter()
        .find_map(|(item, definition)| {
            matches!(definition.key(), ExecutableItemKey::Closure(_)).then_some(item)
        })
        .unwrap();
    let generic_caller = program.functions().iter().find_map(|(_, function)| {
        function
            .operations()
            .iter()
            .any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Call(call)
                        if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
                )
            })
            .then_some(function)
    });
    let generic_caller = generic_caller.unwrap();
    let operations = generic_caller
        .operations()
        .iter()
        .map(|(_, operation)| operation.kind())
        .collect::<Vec<_>>();
    let call = operations.iter().position(|operation| {
        matches!(
            operation,
            MirOperationKind::Call(call)
                if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
        )
    });
    let drop = operations.iter().position(|operation| {
        let MirOperationKind::InvokeDrop { place, .. } = operation else {
            return false;
        };
        generic_caller
            .places()
            .get(*place)
            .is_some_and(place_has_capture_projection)
    });

    assert!(call.zip(drop).is_some_and(|(call, drop)| call < drop));
}

#[test]
fn owned_closure_operand_is_staged_until_propagating_arguments_succeed() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func invoke(input: Owned?): Owned? {\n\
             let captured = Owned { value: 7 }\n\
             let callback = (move captured; item: Owned): Owned {\n\
                 let _ = item.value\n\
                 move captured\n\
             }\n\
             callback(move input?)\n\
         }\n\
         func main(): void {\n\
             let _ = invoke(none)\n\
             return\n\
         }\n",
    )
    .unwrap();
    let closure_item = program
        .executable()
        .items()
        .iter()
        .find_map(|(item, definition)| {
            matches!(definition.key(), ExecutableItemKey::Closure(_)).then_some(item)
        })
        .unwrap();
    let caller = program
        .functions()
        .iter()
        .find_map(|(_, function)| {
            function
                .operations()
                .iter()
                .any(|(_, operation)| {
                    matches!(
                        operation.kind(),
                        MirOperationKind::Call(call)
                            if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
                    )
                })
                .then_some(function)
        })
        .unwrap();

    assert!(caller.operations().iter().any(|(_, operation)| {
        let MirOperationKind::InvokeDrop { place, .. } = operation.kind() else {
            return false;
        };
        caller
            .places()
            .get(*place)
            .is_some_and(place_has_capture_projection)
    }));
    assert!(caller.operations().iter().any(|(_, operation)| {
        matches!(operation.kind(), MirOperationKind::Initialize { destination, .. }
            if caller.places().get(*destination).is_some_and(|place| place.projections().is_empty()))
    }));
}

#[test]
fn owned_callable_bound_preserves_the_same_argument_failure_cleanup() {
    let program = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func invoke<F>(callback: F, input: Owned?): Owned? where F: func(value: Owned): Owned {\n\
             callback(move input?)\n\
         }\n\
         func main(): void {\n\
             let captured = Owned { value: 7 }\n\
             let _ = invoke((move captured; item) {\n\
                 let _ = item.value\n\
                 move captured\n\
             }, none)\n\
             return\n\
         }\n",
    )
    .unwrap();
    let closure_item = program
        .executable()
        .items()
        .iter()
        .find_map(|(item, definition)| {
            matches!(definition.key(), ExecutableItemKey::Closure(_)).then_some(item)
        })
        .unwrap();
    let caller = program
        .functions()
        .iter()
        .find_map(|(_, function)| {
            let invokes_closure = function.operations().iter().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MirOperationKind::Call(call)
                        if matches!(call.target(), crate::MirCallTarget::Direct(body) if *body == closure_item)
                )
            });
            let cleans_staged_capture = function.operations().iter().any(|(_, operation)| {
                let MirOperationKind::InvokeDrop { place, .. } = operation.kind() else {
                    return false;
                };
                function
                    .places()
                    .get(*place)
                    .is_some_and(place_has_capture_projection)
            });
            (invokes_closure && cleans_staged_capture).then_some(function)
        })
        .unwrap();

    assert!(caller.operations().iter().any(|(_, operation)| {
        matches!(operation.kind(), MirOperationKind::Initialize { destination, .. }
            if caller.places().get(*destination).is_some_and(|place| place.projections().is_empty()))
    }));
}

fn only_closure_function(program: &crate::MirProgram) -> &crate::MirFunction {
    program
        .functions()
        .iter()
        .find(|(item, _)| {
            matches!(
                program
                    .executable()
                    .items()
                    .get(*item)
                    .map(nocter_target_program::ExecutableItem::key),
                Some(ExecutableItemKey::Closure(_))
            )
        })
        .expect("fixture must produce one closure function")
        .1
}

fn place_has_capture_projection(place: &crate::MirPlace) -> bool {
    place
        .projections()
        .iter()
        .any(|projection| matches!(projection.kind(), MirProjectionKind::ClosureCapture(_)))
}

fn lower_fixture(source: &str) -> Result<crate::MirProgram, MirLoweringError> {
    lower_compiler_fixture(&CompilerFixture::with_app(source))
}

fn lower_fixture_with_uses(
    source: &str,
    modules: &[&[&str]],
) -> Result<crate::MirProgram, MirLoweringError> {
    lower_compiler_fixture(&CompilerFixture::with_app_standard_uses(source, modules))
}

fn lower_compiler_fixture(
    fixture: &CompilerFixture,
) -> Result<crate::MirProgram, MirLoweringError> {
    lower_executable(executable_fixture(fixture))
}

fn executable_fixture(fixture: &CompilerFixture) -> ExecutableProgram {
    let (target, selected) = target_fixture(fixture);
    ExecutableProgram::for_executable(target, selected).unwrap()
}

fn test_executable_fixture(fixture: &CompilerFixture) -> ExecutableProgram {
    let (target, selected) = target_fixture(fixture);
    ExecutableProgram::for_tests(target, selected).unwrap()
}

fn target_fixture(fixture: &CompilerFixture) -> (TargetProgram, nocter_model::PackageTargetId) {
    let input = fixture.input();
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (declarations, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, declarations, source_index).unwrap();
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
    (target, selected)
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
