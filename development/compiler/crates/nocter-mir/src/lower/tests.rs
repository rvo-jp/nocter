use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_model::CompilationTarget;
use nocter_target_program::{
    ExecutableProgram, PrimitiveBinding, PrimitiveRegistry, PrimitiveRole, TargetProgram,
    ToolchainSnapshot,
};
use nocter_test_support::CompilerFixture;

use super::{MirLoweringError, lower_executable};
use crate::{MirOperationKind, MirTerminator};

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
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
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
    ExecutableProgram::for_executable(target, selected).unwrap()
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
