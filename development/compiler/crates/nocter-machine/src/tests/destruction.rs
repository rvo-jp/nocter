use nocter_runtime_contract::PrimitiveRole;
use nocter_test_support::CompilerFixture;

use super::lower_selected_fixture;
use crate::{MachineContextRequirement, MachineOperationKind, MachineProgram, MachineTerminator};

#[test]
fn primitive_destruction_becomes_one_generated_machine_function() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         func main(): i32 {\n\
             var value = Resource {}\n\
             let pointer = from_ref_mut(&+value)\n\
             drop_value_at_ptr_for_test(pointer, 0)\n\
             return 0\n\
         }\n",
        &[&["internal", "ptr"], &["ptr"]],
    );
    let program = MachineProgram::lower(&lower_selected_fixture(&fixture, false)).unwrap();
    let function_id = generated_destruction_functions(&program)
        .next()
        .expect("concrete destruction function");
    assert_eq!(generated_destruction_functions(&program).count(), 1);
    let function = program.function(function_id).unwrap();
    assert_eq!(function.body().parameters().len(), 2);
    assert!(function.body().operations().any(|(_, operation)| {
        matches!(operation.kind(), MachineOperationKind::InvokeDrop { .. })
    }));
    assert!(
        program
            .functions()
            .flat_map(|(_, function)| function.body().operations())
            .any(|(_, operation)| matches!(
                operation.kind(),
                MachineOperationKind::Call(call)
                    if call.target() == &crate::MachineCallTarget::Direct(function_id)
            ))
    );
    assert!(!program.functions().any(|(_, function)| {
        function.body().operations().any(|(_, operation)| {
            matches!(
                operation.kind(),
                MachineOperationKind::Call(call)
                    if matches!(
                        call.target(),
                        crate::MachineCallTarget::Primitive(primitive)
                            if primitive.role() == PrimitiveRole::DropValueAtPointer
                                && matches!(
                                    primitive.dependency(),
                                    crate::MachinePrimitiveDependency::Destruction {
                                        plan: Some(_), ..
                                    }
                                )
                    )
            )
        })
    }));
}

#[test]
fn generated_fixed_array_destruction_uses_a_reverse_loop() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         struct Container { values: [Resource; 2] }\n\
         func main(): i32 {\n\
             var value = Container { values: [Resource {}, Resource {}] }\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return 0\n\
         }\n",
        &[&["internal", "ptr"], &["ptr"]],
    );
    let program = MachineProgram::lower(&lower_selected_fixture(&fixture, false)).unwrap();
    let function_id = generated_destruction_functions(&program)
        .next()
        .expect("generated array destruction");
    let function = program.function(function_id).unwrap();

    assert!(
        function
            .body()
            .blocks()
            .any(|(_, block)| { matches!(block.terminator(), MachineTerminator::Branch { .. }) })
    );
    assert!(function.body().addresses().any(|(_, address)| {
        address
            .steps()
            .iter()
            .any(|step| matches!(step, crate::MachineAddressStep::OffsetValue(_)))
    }));
    assert_eq!(
        function
            .body()
            .operations()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MachineOperationKind::InvokeDrop { .. })
            })
            .count(),
        1,
        "array length must not unroll destruction code"
    );
}

#[test]
fn generated_destruction_propagates_user_drop_allocation_context() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/mem.allocation_context_state_for_test\n\
         use std/internal/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct Resource {}\n\
         drop Resource(&+self) {\n\
             let _ = allocation_context_state_for_test()\n\
             return\n\
         }\n\
         func main(): i32 {\n\
             var value = Resource {}\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return 0\n\
         }\n",
        &[&["mem"], &["internal", "ptr"], &["ptr"]],
    );
    let program = MachineProgram::lower(&lower_selected_fixture(&fixture, false)).unwrap();
    let function = generated_destruction_functions(&program)
        .next()
        .expect("generated destruction function");

    assert_eq!(
        program.contexts().allocation().get(function),
        Some(MachineContextRequirement::Incoming)
    );
}

fn generated_destruction_functions(
    program: &MachineProgram,
) -> impl Iterator<Item = crate::MachineFunctionId> + '_ {
    program.functions().filter_map(|(id, function)| {
        (function.body().parameters().len() == 2
            && function.body().operations().any(|(_, operation)| {
                matches!(operation.kind(), MachineOperationKind::InvokeDrop { .. })
            }))
        .then_some(id)
    })
}
