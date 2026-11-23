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
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let frames = received_frames(&unit);
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

#[test]
fn nested_returned_frame_control_work_failure_publishes_no_partial_selection() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-loop-exit-results.nct"
    )));
    let unit = SourceUnit::prepare(fixture.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = program.bind_code(code("calculate").into(), []).unwrap();
    let body = unit.code_body(caller.code()).unwrap();
    let call = body.typed().body().expressions().iter().find_map(|(call, expression)|
        matches!(expression.kind, ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), .. }
            if target == code("select"))
            .then_some(ValueApplication { owner: caller.code(), call })).unwrap();
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let returned_frame = unit
        .instantiate_invocation(&caller, call, &mut healthy())
        .unwrap();
    let control = unit.instantiate_control(&caller, &mut healthy()).unwrap();
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    let mut work = healthy();
    let mut query = Query::with_control(program, evidence, &mut work, &control);
    for steps in [1, 2] {
        *query.work = WorkBudget::new(NonZeroU64::new(steps).unwrap());
        assert!(matches!(
            query.control_for(&returned_frame),
            Err(InstantiationFailure::WorkLimit)
        ));
        assert!(!query.controls.contains_key(&returned_frame));
    }
    *query.work = healthy();
    let selected = query.control_for(&returned_frame).unwrap().unwrap();
    assert_eq!(selected.frame(), &returned_frame);
    let (point, block) = unit
        .code_body(returned_frame.code())
        .unwrap()
        .graph()
        .points()
        .iter()
        .find(|(_, block)| {
            matches!(
                block.terminator,
                crate::value_access::flow::Terminator::Branch { .. }
            )
        })
        .unwrap();
    let crate::value_access::flow::Terminator::Branch { no, .. } = block.terminator else {
        unreachable!()
    };
    assert_eq!(
        selected.terminator(point),
        Some(crate::value_access::flow::Terminator::Jump(no))
    );
    assert!(query.call(&caller, value_call(body)).is_ok());
}

#[test]
fn selected_return_data_excludes_inactive_normal_completion_boundaries() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-return-boundaries.nct"
    )));
    let unit = SourceUnit::prepare(fixture.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let frame = program.bind_code(code("calculate").into(), []).unwrap();
    let body = unit.code_body(frame.code()).unwrap();
    let call = body.typed().body().expressions().iter().find_map(|(call, expression)|
        matches!(expression.kind, ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), .. }
            if target == code("select"))
            .then_some(ValueApplication { owner: frame.code(), call })).unwrap();
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let selected = unit
        .instantiate_invocation(&frame, call, &mut healthy())
        .unwrap();
    let sources = unit.value_sources(selected.code()).unwrap();
    assert_eq!(
        sources.returned().len(),
        2,
        "both original staged return definitions survive"
    );
    let body = unit.code_body(selected.code()).unwrap();
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    let result = Key::DataResult(
        selected.clone(),
        ValuePath::default(),
        body.typed().result_data(),
    );
    let conservative = Query::new(program, evidence, &mut healthy())
        .solve(&result)
        .unwrap();
    let Fact::Data(conservative) = conservative else {
        panic!("represented result domain")
    };
    let InstanceSchema::FunctionAlternatives(choices) = conservative.schema(program).unwrap()
    else {
        panic!("unselected return alternatives")
    };
    assert_eq!(choices.len(), 2);
    assert!(conservative.function_representation(program).is_err());
    let data = unit
        .instantiate_result_contract(&selected, &mut healthy())
        .unwrap();
    let InstanceSchema::Function(witness) = data.schema(program).unwrap() else {
        panic!("callable result")
    };
    assert!(choices.contains(&witness));
    let invoked = unit
        .instantiate_invocation(
            &frame,
            value_call(unit.code_body(frame.code()).unwrap()),
            &mut healthy(),
        )
        .unwrap();
    assert_eq!(invoked.code(), witness.code());
    assert_eq!(invoked.environment(), witness.environment());
    let control = unit.instantiate_control(&selected, &mut healthy()).unwrap();
    assert!(
        sources.returned().iter().any(|&value| sources
            .node(value)
            .unwrap()
            .origin
            .is_some_and(|origin| !control.reachable(origin.point))),
        "an original staged return lies on an excluded normal-completion path"
    );
}

#[test]
fn public_loop_exit_queries_keep_one_callable_witness_and_referent_root() {
    use crate::value_access::ArgumentAction;
    let unit = Arc::new(source_unit());
    let frame = received_frames(&unit).remove(0);
    let body = unit.code_body(frame.code()).unwrap();
    let call = value_call(body);
    let graph = unit.value_sources(frame.code()).unwrap();
    let [bundle] = graph.call(call.call).unwrap() else {
        panic!("one original call bundle")
    };
    let ExpressionKind::Call { arguments, .. } = &body
        .typed()
        .body()
        .expressions()
        .get(call.call)
        .unwrap()
        .kind
    else {
        panic!("call")
    };
    let ArgumentAction::ReserveRead(referent) = arguments[0].action else {
        panic!("a read receiver")
    };
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    let target = unit
        .instantiate_invocation(&frame, call, &mut healthy())
        .unwrap();
    let value = unit.instantiate_value(
        &frame,
        bundle.callee.witness_source().unwrap(),
        &mut healthy(),
    );
    let root = unit.instantiate_referent_root_data(&frame, bundle.origin, referent, &mut healthy());
    let after =
        unit.instantiate_referent_root_data_after(&frame, bundle.origin, referent, &mut healthy());
    assert!(
        value.is_ok() && root.is_ok() && after.is_ok(),
        "witness: {value:?}; referent: {root:?}; after: {after:?}"
    );
    let value = value.unwrap();
    let root = root.unwrap();
    assert_eq!(after.unwrap(), root);
    let InstanceSchema::Function(backing) = root.schema(unit.program()).unwrap() else {
        panic!("callable root")
    };
    assert_eq!(value.code(), target.code());
    assert_eq!(value.environment(), target.environment());
    assert_eq!(backing.code(), value.code());
    assert_eq!(backing.environment(), value.environment());
    let metadata = unit.instance_metadata(&frame, &mut healthy()).unwrap();
    assert_eq!(
        metadata
            .value(bundle.callee.witness_source().unwrap(), &mut healthy())
            .unwrap(),
        value
    );
}

fn received_frames(unit: &crate::value_access::accepted::Unit) -> Vec<CodeInstanceKey> {
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("contexts").into(), []).unwrap();
    let healthy = || WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
    unit.code_body(root.code())
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
        .collect::<Vec<_>>()
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
