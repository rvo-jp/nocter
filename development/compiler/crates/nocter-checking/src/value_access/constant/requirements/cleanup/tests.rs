//! Original cleanup roles, independently of source admission of contextual payloads.
use super::*;
use crate::value_access::flow::resources::{self, Holder};
use nocter_model::{ArenaBuilder, BuiltinType, ValueFunctionOrigin, ValueInvocation};
use nocter_test_support::ValueSourceFixture;

#[test]
fn scoped_cleanup_conditions_retain_the_original_retired_step_and_reject_invalid_cursors() {
    let source = ValueSourceFixture::new(
        "func replace(edit target: error, take next: error): void { target = next^ }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(source.syntax())
        .unwrap()
        .into_unit();
    let code = unit
        .program()
        .resolve_code(unit.program().symbols().get("replace").unwrap())
        .unwrap();
    let body = unit.code_body(code.into()).unwrap();
    let mut domain = evaluation::Types::default();
    let conditions = collect(body, &mut domain).unwrap();
    let mut replacements = 0;
    for condition in conditions {
        let original = condition.original(body).unwrap();
        assert_eq!(
            condition.data(body).unwrap(),
            payload(original).unwrap().data()
        );
        if let Step::ReplaceReferent {
            retired,
            payload: supplied,
            ..
        } = original
        {
            assert_eq!(retired.data(), supplied.data());
            assert!(std::ptr::eq(payload(original).unwrap(), retired));
            assert!(!std::ptr::eq(payload(original).unwrap(), supplied));
            let Site::Cleanup(at) = condition.violation().site else {
                panic!("cleanup")
            };
            assert!(matches!(
                body.graph().points().get(at.point).unwrap().actions[at.action].operation,
                flow::Operation::Assign { .. }
            ));
            replacements += 1;
        }
        let mut invalid = condition.clone();
        invalid.step = usize::MAX;
        assert_eq!(invalid.data(body), Err(INVALID));
        let mut invalid = condition.clone();
        invalid.transition = usize::MAX;
        assert_eq!(invalid.data(body), Err(INVALID));
        let mut invalid = condition;
        invalid.violation.rule =
            Rule::UnsupportedData(unit.program().data_type(BuiltinType::Error).unwrap());
        assert_eq!(invalid.data(body), Err(INVALID));
    }
    assert_eq!(replacements, 1);
}

#[test]
fn projected_cleanup_selects_the_retired_context_when_receiving_the_same_source_type() {
    use nocter_declarations::value_access::FunctionWitness;
    let source = ValueSourceFixture::new(
        "func first(): void {}\nfunc second(): void {}\nfunc root(take action: func(): void): void { drop action }",
    );
    let declarations =
        nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax()).unwrap();
    let context = declarations.bind(source.syntax()).unwrap();
    let program = context.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let parameter = program.codes().get(code("root")).unwrap().parameters()[0];
    let data = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter.into()),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let keys = ["first", "second"].map(|name| {
        program
            .instantiate_function(data, &program.concrete_witness(code(name).into()).unwrap())
            .unwrap()
    });
    assert_eq!(keys[0].data(), keys[1].data());
    assert_ne!(keys[0], keys[1]);
    let contract = resources::payload(program, data).unwrap();
    let [retired, supplied] = keys
        .each_ref()
        .map(|key| contract.instantiate(program, key).unwrap());
    let mut expressions = ArenaBuilder::new();
    let step = Step::ReplaceReferent {
        destination: Holder::Return.place(),
        source: Holder::Expression(expressions.insert(())).place(),
        retired: retired.clone(),
        payload: supplied.clone(),
    };
    assert_eq!(payload(&step).unwrap().instance(), retired.instance());
    assert_ne!(payload(&step).unwrap().instance(), supplied.instance());
}

#[test]
fn bound_cleanup_uses_the_received_environment_and_retries_after_work_failure() {
    use crate::value_access::constant::{BoundCheck, BoundCheckFailure};
    use crate::value_access::{ExpressionKind, InstantiationFailure};
    use nocter_constant_evaluation::state::{Limits, WorkBudget};
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::ValueApplication;
    use std::num::NonZeroU64;

    let source = ValueSourceFixture::new(
        "func first(): void {}\nconst func discard(take action: func(): void): void {}\nfunc run(take held: error): void { let action = (held^;): void {}\ndiscard(action^) }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(source.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let discard = code("discard");
    let formal = program.codes().get(discard).unwrap().parameters()[0];
    let plain = program
        .bind_code(
            discard.into(),
            [(formal.into(), FunctionWitness::from(code("first")))],
        )
        .unwrap();
    let run = unit.code_body(code("run").into()).unwrap();
    let (call, _) = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
        .unwrap();
    let healthy = || WorkBudget::new(Limits::default().steps());
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
    let requirements = unit.const_requirements(discard.into()).unwrap();
    let body = requirements.body();
    let conditions = collect(body, &mut evaluation::Types::default()).unwrap();
    assert_eq!(conditions.len(), 1);
    let condition = &conditions[0];
    let (at, transition) = condition.request().unwrap();
    let keys = [&plain, &captured].map(|frame| {
        let projected = project(body, frame, at, transition, &mut healthy()).unwrap();
        condition.projected_data(&projected).unwrap()
    });
    // The original condition keeps its formal source type. Resource payloads normalize to
    // each selected function's concrete representation; those raw identities need not agree.
    assert_ne!(keys[0].data(), keys[1].data());
    assert_ne!(keys[0], keys[1]);
    let mut domain = evaluation::Types::default();
    assert!(domain.supports_instance(program, keys[0].clone()).unwrap());
    assert!(!domain.supports_instance(program, keys[1].clone()).unwrap());
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    exhausted.step().unwrap();
    assert!(
        matches!(requirements.check_bound(&|_| true, &plain, &mut exhausted),
        Err(BoundCheckFailure::Data { at: failed_at, failure: InstantiationFailure::WorkLimit }) if failed_at == at)
    );
    assert!(
        matches!(requirements.check_bound(&|_| true, &plain, &mut healthy()).unwrap(), BoundCheck::Calls(calls) if calls.is_empty())
    );
    let BoundCheck::Violation(violation) = requirements
        .check_bound(&|_| true, &captured, &mut healthy())
        .unwrap()
    else {
        panic!("owned error environment must remain outside the const domain")
    };
    assert_eq!(violation.rule(), condition.violation().rule());
    assert!(matches!(violation.site, Site::Cleanup(actual) if actual == at));
}
