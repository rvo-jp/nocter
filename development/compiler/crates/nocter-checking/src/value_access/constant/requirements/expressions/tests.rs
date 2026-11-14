//! A formal expression contract does not identify its concrete value environment.
use super::*;
use crate::value_access::{ExpressionKind, InstantiationFailure, accepted::SourceUnit};
use nocter_constant_evaluation::state::Limits;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::ValueApplication;
use nocter_test_support::ValueSourceFixture;
use std::num::NonZeroU64;

#[test]
fn expression_conditions_select_original_values_and_preserve_work_failure_locations() {
    let source = ValueSourceFixture::new(
        "func first(): void {}\nconst func inspect(action: func(): void): void { read alias = action }\nfunc run(take held: error): void { let action = (held^;): void {}\ninspect(action) }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let inspect = code("inspect");
    let body = unit.code_body(inspect.into()).unwrap();
    let conditions = collect(body, &mut evaluation::Types::default()).unwrap();
    assert!(!conditions.is_empty());
    let formal = program.codes().get(inspect).unwrap().parameters()[0];
    let plain = program
        .bind_code(
            inspect.into(),
            [(formal.into(), FunctionWitness::from(code("first")))],
        )
        .unwrap();
    let healthy = || WorkBudget::new(Limits::default().steps());
    let run = unit.code_body(code("run").into()).unwrap();
    let (call, _) = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
        .unwrap();
    let captured = unit
        .instantiate_invocation(
            &program.bind_code(run.typed().code(), []).unwrap(),
            ValueApplication {
                owner: run.typed().code(),
                call,
            },
            &mut healthy(),
        )
        .unwrap();
    for condition in conditions.values() {
        assert!(!condition.selections.is_empty());
        let mut domain = evaluation::Types::default();
        assert!(
            condition
                .supports(body, &plain, &mut healthy(), &mut domain)
                .unwrap()
        );
        assert!(
            !condition
                .supports(body, &captured, &mut healthy(), &mut domain)
                .unwrap()
        );
        let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
        exhausted.step().unwrap();
        assert!(
            matches!(condition.supports(body, &plain, &mut exhausted, &mut domain),
            Err(BoundCheckFailure::Data { at, failure: InstantiationFailure::WorkLimit }) if at == condition.selections[0].0)
        );
        assert!(
            condition
                .supports(body, &plain, &mut healthy(), &mut domain)
                .unwrap()
        );
        let wrong = program.bind_code(code("first").into(), []).unwrap();
        assert!(matches!(
            condition.supports(body, &wrong, &mut healthy(), &mut domain),
            Err(BoundCheckFailure::Integrity(_))
        ));
    }
}

#[test]
fn dormant_expressions_retain_definition_contract_checks_without_source_values() {
    let source = ValueSourceFixture::new(
        "func first(): void {}\nconst func inspect(action: func(): void): void { return ()\nread alias = action }\nfunc run(take held: error): void { let action = (held^;): void {}\ninspect(action) }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let inspect = code("inspect");
    let body = unit.code_body(inspect.into()).unwrap();
    let conditions = collect(body, &mut evaluation::Types::default()).unwrap();
    assert!(!conditions.is_empty());
    let formal = program.codes().get(inspect).unwrap().parameters()[0];
    let frame = program
        .bind_code(
            inspect.into(),
            [(formal.into(), FunctionWitness::from(code("first")))],
        )
        .unwrap();
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    exhausted.step().unwrap();
    for condition in conditions.values() {
        assert!(condition.selections.is_empty());
        assert!(
            condition
                .supports(
                    body,
                    &frame,
                    &mut exhausted,
                    &mut evaluation::Types::default()
                )
                .unwrap()
        );
    }
    let run = unit.code_body(code("run").into()).unwrap();
    let (call, _) = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
        .unwrap();
    let captured = unit
        .instantiate_invocation(
            &program.bind_code(run.typed().code(), []).unwrap(),
            ValueApplication {
                owner: run.typed().code(),
                call,
            },
            &mut WorkBudget::new(Limits::default().steps()),
        )
        .unwrap();
    let mut domain = evaluation::Types::default();
    for condition in conditions.values() {
        assert!(
            condition
                .supports(body, &captured, &mut exhausted, &mut domain)
                .unwrap()
        );
        assert!(
            !domain
                .supports_input(program, &captured, 0, condition.data(body).unwrap())
                .unwrap()
        );
    }
}
