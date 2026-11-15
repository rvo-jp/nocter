//! Selected original edges refine one exact request, never another received input domain.
use super::*;
use crate::value_access::{CallKind, ExpressionKind, accepted::SourceUnit};
use nocter_declarations::value_access::InstanceSchema;
use nocter_model::ValueFunctionOrigin;
use nocter_test_support::ValueSourceFixture;

fn source_unit() -> crate::value_access::accepted::Unit {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-loop-exit-contexts.nct"
    ));
    let source = source.replace("const ANSWER: i32 = choose(absent())", "");
    let source = format!(
        "{source}\nconst func present(): (func(i32): i32)? {{ identity }}\nfunc contexts(): void {{ let _ = choose(absent())\nlet _ = choose(present()) }}"
    );
    let fixture = ValueSourceFixture::new(&source);
    SourceUnit::prepare(fixture.syntax()).unwrap().into_unit()
}

#[test]
fn selected_loop_exit_queries_keep_frame_boundaries_and_recover_after_work_failure() {
    let unit = source_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("contexts").into(), []).unwrap();
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let frames = unit
        .code_body(root.code())
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(call, expression)| {
            matches!(expression.kind,
            ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), .. }
                if target == code("choose"))
            .then_some(ValueApplication {
                owner: root.code(),
                call,
            })
        })
        .map(|site| {
            unit.instantiate_invocation(&root, site, &mut healthy())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let [empty, positive] = frames.as_slice() else {
        panic!("two original received contexts")
    };
    assert_eq!(empty.code(), positive.code());
    assert_ne!(empty, positive);
    let body = unit.code_body(empty.code()).unwrap();
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    let call = value_call(body);
    let control = unit.instantiate_control(empty, &mut healthy()).unwrap();
    let other = unit.instantiate_control(positive, &mut healthy()).unwrap();
    assert_eq!(
        super::super::call(program, evidence, empty, call, &mut healthy()),
        Err(InstantiationFailure::Unresolved)
    );
    let selected =
        call_with_control(program, evidence, empty, call, &control, &mut healthy()).unwrap();
    assert_eq!(
        call_with_control(program, evidence, positive, call, &other, &mut healthy()),
        Err(InstantiationFailure::Unresolved),
        "an inhabited domain retains both possible exits"
    );
    assert_eq!(
        call_with_control(program, evidence, positive, call, &control, &mut healthy()),
        Err(InstantiationFailure::WrongOwner)
    );
    let first = body.typed().body().expressions().iter().find_map(|(call, expression)|
        matches!(expression.kind, ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), .. }
            if target == code("wrap"))
            .then_some(ValueApplication { owner: empty.code(), call })).unwrap();
    let initial = unit
        .instantiate_invocation(empty, first, &mut healthy())
        .unwrap();
    let data = unit
        .instantiate_result_contract(&initial, &mut healthy())
        .unwrap();
    let InstanceSchema::Function(initial) = data.schema(program).unwrap() else {
        panic!("initial callable")
    };
    assert_eq!(selected.code(), initial.code());
    assert_ne!(
        selected.environment(),
        initial.environment(),
        "the reachable initial producer is not the selected exit's current value"
    );
    let graph = evidence.value_sources(empty.code()).unwrap();
    assert!(
        graph
            .call(first.call)
            .unwrap()
            .iter()
            .all(|bundle| control.reachable(bundle.origin.point))
    );
    assert_work_recovery(program, evidence, empty, positive, call, &control, selected);
}

fn value_call(body: &crate::value_access::flow::Query) -> ValueApplication {
    body.typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(call, expression)| {
            matches!(
                expression.kind,
                ExpressionKind::Call {
                    kind: CallKind::Value,
                    ..
                }
            )
            .then_some(ValueApplication {
                owner: body.typed().code(),
                call,
            })
        })
        .unwrap()
}

fn assert_work_recovery(
    program: &Program,
    evidence: &Evidence,
    empty: &CodeInstanceKey,
    positive: &CodeInstanceKey,
    call: ValueApplication,
    control: &crate::value_access::InstanceControl,
    selected: CodeInstanceKey,
) {
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let mut work = healthy();
    let mut query = Query::with_control(program, evidence, &mut work, control);
    for units in [1, 2] {
        *query.work = WorkBudget::new(NonZeroU64::new(units).unwrap());
        assert_eq!(
            query.call(empty, call),
            Err(InstantiationFailure::WorkLimit)
        );
        assert!(query.entries.is_empty());
        assert!(query.pending.is_empty());
    }
    *query.work = healthy();
    assert_eq!(query.call(empty, call), Ok(selected));
    assert_eq!(
        query.call(positive, call),
        Err(InstantiationFailure::Unresolved),
        "a different frame cannot borrow this query's selected edges"
    );
}
