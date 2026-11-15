//! Real source frames select continuations without changing the shared checked graph.
use super::*;
use nocter_model::{ValueApplication, ValueFunctionOrigin as Origin};
use std::num::NonZeroU64;

fn source_unit() -> crate::value_access::accepted::Unit {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-control.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit()
}

#[test]
fn empty_and_positive_received_domains_select_distinct_original_continuations() {
    let unit = source_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("calculate").into(), []).unwrap();
    let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    let body = unit.code_body(code("inspect").into()).unwrap();
    let (point, original) = body
        .graph()
        .points()
        .iter()
        .find(|(_, point)| matches!(point.terminator, flow::Terminator::Branch { .. }))
        .unwrap();
    let flow::Terminator::Branch { no, .. } = original.terminator else {
        unreachable!()
    };
    let mut frames = Vec::new();
    for (call, expression) in unit
        .code_body(root.code())
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
    {
        if !matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code("inspect"))
        {
            continue;
        }
        frames.push(
            unit.instantiate_invocation(
                &root,
                ValueApplication {
                    owner: root.code(),
                    call,
                },
                &mut work,
            )
            .unwrap(),
        );
    }
    assert_eq!(frames.len(), 2);
    let absent = unit.instantiate_control(&frames[0], &mut work).unwrap();
    let positive = unit.instantiate_control(&frames[1], &mut work).unwrap();
    assert_eq!(absent.frame(), &frames[0]);
    assert_eq!(positive.frame(), &frames[1]);
    assert_eq!(absent.terminator(point), Some(flow::Terminator::Jump(no)));
    assert_eq!(positive.terminator(point), Some(original.terminator));
    let (call, _) = body
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| {
            matches!(
                expression.kind,
                ExpressionKind::Call {
                    target: Origin::Parameter(_),
                    ..
                }
            )
        })
        .unwrap();
    let invocation = &unit
        .value_sources(frames[0].code())
        .unwrap()
        .call(call)
        .unwrap()[0];
    assert!(!absent.reachable(invocation.origin.point));
    assert!(positive.reachable(invocation.origin.point));
    assert!(
        !absent
            .call_reachable(unit.value_sources(frames[0].code()).unwrap(), call)
            .unwrap()
    );
    assert!(
        absent
            .call_reachable(unit.value_sources(root.code()).unwrap(), call)
            .is_err()
    );
    assert_eq!(
        body.graph().points().get(point).unwrap().terminator,
        original.terminator
    );
    assert!(matches!(
        unit.instantiate_control(
            &frames[0],
            &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
        ),
        Err(Failure::WorkLimit)
    ));
    assert_eq!(
        unit.instantiate_control(&frames[0], &mut work)
            .unwrap()
            .terminator(point),
        absent.terminator(point)
    );
}
