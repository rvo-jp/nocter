//! Source-selected explicit retirement and dormant declaration contracts are different checks.
use super::*;
use crate::value_access::{ExpressionKind, InstantiationFailure, accepted::SourceUnit};
use nocter_constant_evaluation::state::Limits;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::ValueApplication;
use nocter_test_support::ValueSourceFixture;
use std::num::NonZeroU64;

#[test]
fn explicit_drop_selects_the_original_local_before_retirement_and_recovers_from_work_failure() {
    let source = ValueSourceFixture::new(
        "func first(): void {}\nconst func discard(take action: func(): void): void { let next = action^\ndrop next }\nfunc run(take held: error): void { let action = (held^;): void {}\ndiscard(action^) }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let discard = code("discard");
    let body = unit.code_body(discard.into()).unwrap();
    let conditions = collect(body, &mut evaluation::Types::default()).unwrap();
    assert_eq!(conditions.len(), 1);
    let condition = &conditions[0];
    assert_eq!(condition.selections.len(), 1);
    let at = condition.selections[0];
    let formal = program.codes().get(discard).unwrap().parameters()[0];
    let plain = program
        .bind_code(
            discard.into(),
            [(formal.into(), FunctionWitness::from(code("first")))],
        )
        .unwrap();
    let healthy = || WorkBudget::new(Limits::default().steps());
    let mut domain = evaluation::Types::default();
    assert!(
        condition
            .supports(body, &plain, &mut healthy(), &mut domain)
            .unwrap()
    );
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
    assert!(
        !condition
            .supports(body, &captured, &mut healthy(), &mut domain)
            .unwrap()
    );
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    exhausted.step().unwrap();
    assert!(
        matches!(condition.supports(body, &plain, &mut exhausted, &mut domain),
        Err(BoundCheckFailure::Data { at: actual, failure: InstantiationFailure::WorkLimit }) if actual == at)
    );
    assert!(
        condition
            .supports(body, &plain, &mut healthy(), &mut domain)
            .unwrap()
    );
    let mut invalid = condition.clone();
    invalid.selections[0].action = usize::MAX;
    assert!(matches!(
        invalid.supports(body, &plain, &mut healthy(), &mut domain),
        Err(BoundCheckFailure::Integrity(_))
    ));
    let wrong = program.bind_code(code("first").into(), []).unwrap();
    assert!(matches!(
        condition.supports(body, &wrong, &mut healthy(), &mut domain),
        Err(BoundCheckFailure::Integrity(_))
    ));
}

#[test]
fn dormant_drop_preserves_its_contract_without_inventing_a_source_occurrence() {
    let source = ValueSourceFixture::new(
        "func first(): void {}\nconst func dormant(take action: func(): void): void { return ()\ndrop action }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let dormant = code("dormant");
    let body = unit.code_body(dormant.into()).unwrap();
    let conditions = collect(body, &mut evaluation::Types::default()).unwrap();
    assert_eq!(conditions.len(), 1);
    assert!(conditions[0].selections.is_empty());
    let formal = program.codes().get(dormant).unwrap().parameters()[0];
    let frame = program
        .bind_code(
            dormant.into(),
            [(formal.into(), FunctionWitness::from(code("first")))],
        )
        .unwrap();
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    exhausted.step().unwrap();
    assert!(
        conditions[0]
            .supports(
                body,
                &frame,
                &mut exhausted,
                &mut evaluation::Types::default()
            )
            .unwrap()
    );
}
