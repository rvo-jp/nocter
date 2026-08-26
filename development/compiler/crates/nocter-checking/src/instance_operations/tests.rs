use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::CallableKind;
use nocter_model::{BuiltinType, TypeKind};

use super::build_instance_operation_table;
use crate::prepare_program_checking;
use crate::test_support::Fixture;

#[test]
fn overlapping_operation_patterns_are_rejected_before_body_checking() {
    let fixture = Fixture::new(
        "struct Box<T> {}\ninstance Box<T> { method &self.value(): i32 { return 0 } }\ninstance Box<U> where U = i32 { method &self.value(): i32 { return 1 } }\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let error =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0355");
}

#[test]
fn distinct_refined_instance_patterns_share_one_family_index() {
    let fixture = Fixture::new(
        "struct Box<T> {}\ninstance Box<T> where T = i32 {}\ninstance Box<U> where U = u32 {}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, mut types, _admission) = program.into_parts();
    let table =
        build_instance_operation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap();
    let definition = graph
        .declarations()
        .nominal_types()
        .iter()
        .next()
        .unwrap()
        .0;
    let i32_box = types
        .intern(TypeKind::Nominal {
            definition,
            arguments: Box::new([types.builtin(BuiltinType::I32)]),
        })
        .unwrap();

    assert_eq!(table.entries().len(), 2);
    assert_eq!(table.candidates(&types, i32_box).unwrap().len(), 2);
    let first = table.entries().iter().next().unwrap().1;
    let second = table.entries().iter().nth(1).unwrap().1;
    assert_ne!(first.target(), second.target());
    for entry in [first, second] {
        assert_eq!(entry.generic_parameters().len(), 1);
        assert_eq!(entry.refinements().len(), 1);
        let TypeKind::Nominal { arguments, .. } = types.get(entry.target()).unwrap() else {
            panic!("refined instance target must remain nominal");
        };
        assert_eq!(entry.refinements()[0].ty(), arguments[0]);
    }
}

#[test]
fn table_retains_operation_identity_and_normalized_instance_generics() {
    let fixture = Fixture::new(
        "struct Buffer<T> {}\ninstance Buffer<T> {\n    pub operator (&self[index: usize]): &T {\n        loop {}\n    }\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, mut types, _admission) = program.into_parts();
    let table =
        build_instance_operation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap();
    let entry = table.entries().iter().next().unwrap().1;
    let [member] = entry.members() else {
        panic!("expected one indexed operation");
    };

    assert_eq!(
        graph
            .declarations()
            .callables()
            .get(member.callable())
            .unwrap()
            .kind(),
        CallableKind::Index
    );
    assert_eq!(entry.generic_parameters().len(), 1);
    assert!(entry.refinements().is_empty());
}

#[test]
fn duplicate_coercion_identity_is_rejected_before_body_selection() {
    let fixture = Fixture::new(
        "struct View { value: i32 }\nstruct Source { first: View\n    second: View\n}\ninstance Source {\n    pub coerce &self as &View {\n        return &self.first\n    }\n    pub coerce &self as &View {\n        return &self.second\n    }\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let error =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0356");
}

#[test]
fn unused_invalid_instance_operation_is_rejected_during_preparation() {
    let fixture = Fixture::new(
        "struct Buffer { value: i32 }\ninstance Buffer {\n    pub operator (&self[index: usize]): &+i32 {\n        loop {}\n    }\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let error =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0357");
}
