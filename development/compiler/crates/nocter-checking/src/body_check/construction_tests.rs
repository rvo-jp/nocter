use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};
use nocter_source_index::{SemanticEntity, SourceRole};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, CheckedOutcome, OutcomeLayer, prepare_program_checking};

#[test]
fn scalar_local_and_body_result_construct_one_closed_checked_body() {
    let fixture = Fixture::new("func answer(): i32 {\n    let value = 42\n    value\n}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert_eq!(body.locals().len(), 1);
    assert_eq!(
        body.locals().iter().next().unwrap().1.ty(),
        output.program().types().builtin(BuiltinType::I32)
    );
    assert!(body.places().iter().next().unwrap().1.is_move_source());
    assert!(matches!(
        body.nodes().get(body.root()).unwrap().operation(),
        CheckedOperation::Control(_)
    ));
    let root_bindings = output.source_index().bindings_for(SemanticEntity::BodyNode(
        output.program().bodies().iter().next().unwrap().0,
        body.root(),
    ));
    assert!(!root_bindings.is_empty());
    assert!(
        root_bindings
            .iter()
            .all(|binding| binding.role() == SourceRole::Reference)
    );
}

#[test]
fn body_result_materializes_recursive_outcome_injection() {
    let fixture = Fixture::new("func answer(): i32?! {\n    42\n}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let injections = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Outcome(CheckedOutcome::Inject { layer, .. }) => Some(*layer),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        injections,
        vec![OutcomeLayer::Optional, OutcomeLayer::Fallible]
    );
    assert!(matches!(
        output
            .program()
            .types()
            .get(body.nodes().get(body.root()).unwrap().ty()),
        Some(TypeKind::Fallible(_))
    ));
}

#[test]
fn optional_absence_needs_no_synthetic_payload() {
    let fixture = Fixture::new("func answer(): i32? {\n    none\n}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Outcome(CheckedOutcome::Absent)
    )));
}

#[test]
fn reachable_nonvoid_fallthrough_has_one_body_rule() {
    let fixture = Fixture::new("func missing(): i32 {}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0373");
}

#[test]
fn nonfinal_value_expression_is_not_implicitly_discarded() {
    let fixture = Fixture::new("func invalid(): void {\n    42\n    return\n}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0372");
}
