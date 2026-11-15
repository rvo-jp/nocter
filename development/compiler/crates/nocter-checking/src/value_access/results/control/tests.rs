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
fn received_assertions_keep_the_original_check_and_exclude_only_its_continuation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-assertions.nct"
    ));
    for absent in [false, true] {
        let source = source
            .replace("const ANSWER: i32 = calculate()", "")
            .replace("const func", "func");
        let source = if absent {
            source.replace("inspect(present())", "inspect(absent())")
        } else {
            source
        };
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let root = program.bind_code(code("calculate").into(), []).unwrap();
        let body = unit.code_body(code("inspect").into()).unwrap();
        let call = unit.code_body(root.code()).unwrap().typed().body().expressions().iter().find_map(|(call, expression)| matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code("inspect")).then_some(call)).unwrap();
        let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
        let frame = unit
            .instantiate_invocation(
                &root,
                ValueApplication {
                    owner: root.code(),
                    call,
                },
                &mut work,
            )
            .unwrap();
        let selected = unit.instantiate_control(&frame, &mut work).unwrap();
        let (point, check) = body
            .graph()
            .points()
            .iter()
            .find(|(_, point)| {
                flow::assertion_source(body.typed(), point)
                    .unwrap()
                    .is_some()
            })
            .unwrap();
        let flow::Terminator::Jump(next) = check.terminator else {
            unreachable!()
        };
        assert_eq!(
            selected.terminator(point),
            Some(if absent {
                flow::Terminator::Unreachable
            } else {
                check.terminator
            })
        );
        assert_eq!(selected.reachable(next), !absent);
        assert!(
            matches!(check.actions.last().unwrap().operation, flow::Operation::Evaluate(id) if matches!(body.typed().body().expressions().get(id).unwrap().kind, ExpressionKind::RequirePrimary { .. }))
        );
    }
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

#[test]
fn nested_absence_shares_representation_without_erasing_outer_tag_control() {
    use nocter_declarations::value_access::PayloadSchema;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-nested-outcome-control.nct"
    ));
    let source = source.replace("const ANSWER: i32 = calculate()", "");
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("calculate").into(), []).unwrap();
    let caller = unit.code_body(root.code()).unwrap();
    for name in ["inspect", "inspect_read", "inspect_edit"] {
        let target = code(name);
        let body = unit.code_body(target.into()).unwrap();
        let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
        let frames: Vec<_> = caller.typed().body().expressions().iter().filter_map(|(call, expression)| {
            matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(selected), .. } if selected == target).then_some(call)
        }).map(|call| unit.instantiate_invocation(&root, ValueApplication { owner: root.code(), call }, &mut work).unwrap()).collect();
        assert_eq!(frames.len(), 3, "{name}");
        // Outer absence and present-inner-absence have the same represented possibilities.
        assert_eq!(frames[0], frames[1], "{name}");
        assert_ne!(frames[1], frames[2], "{name}");
        for (index, frame) in frames.iter().enumerate() {
            let control = unit.instantiate_control(frame, &mut work).unwrap();
            let mut branches = 0;
            for (point, block) in body.graph().points().iter() {
                let flow::Terminator::Branch { condition, no, .. } = block.terminator else {
                    continue;
                };
                let ExpressionKind::PrimaryCondition { source } = body
                    .typed()
                    .body()
                    .expressions()
                    .get(condition)
                    .unwrap()
                    .kind
                else {
                    panic!("expected an original optional tag condition")
                };
                let data = body.typed().body().referent_data(source).unwrap();
                let Some(PayloadSchema::Optional(primary)) = program.data_schema(data) else {
                    panic!("expected optional data")
                };
                let nested = matches!(
                    program.data_schema(primary),
                    Some(PayloadSchema::Optional(_))
                );
                let expected = if nested || index == 2 {
                    block.terminator
                } else {
                    flow::Terminator::Jump(no)
                };
                assert_eq!(
                    control.terminator(point),
                    Some(expected),
                    "{name}, frame {index}, outer {nested}"
                );
                branches += 1;
            }
            assert_eq!(branches, 2);
        }
    }
}

#[test]
fn compound_received_domains_prune_only_the_selected_component() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-outcome-control.nct"
    ));
    let source = source.replace("const ANSWER: i32 = calculate()", "");
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("calculate").into(), []).unwrap();
    let caller = unit.code_body(root.code()).unwrap();
    for name in ["inspect", "inspect_read", "inspect_edit"] {
        let target = code(name);
        let body = unit.code_body(target.into()).unwrap();
        let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
        let frames: Vec<_> = caller.typed().body().expressions().iter().filter_map(|(call, expression)| {
            matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(selected), .. } if selected == target).then_some(call)
        }).map(|call| unit.instantiate_invocation(&root, ValueApplication { owner: root.code(), call }, &mut work).unwrap()).collect();
        assert_eq!(frames.len(), 2, "{name}");
        assert_ne!(frames[0], frames[1]);
        for (index, frame) in frames.iter().enumerate() {
            let data = frame.input_data(0).unwrap();
            let primary = data
                .component(program, ValueField::Tuple(0))
                .unwrap()
                .component(program, ValueField::Variant(true))
                .unwrap();
            assert_eq!(primary.is_uninhabited(), index == 0);
            let adjacent = data.component(program, ValueField::Tuple(1)).unwrap();
            assert!(adjacent.environment(program).unwrap().is_some());
            assert!(matches!(
                adjacent.schema(program).unwrap(),
                nocter_declarations::value_access::InstanceSchema::Function(_)
            ));
            let control = unit.instantiate_control(frame, &mut work).unwrap();
            let mut branches = 0;
            for (point, block) in body.graph().points().iter() {
                let flow::Terminator::Branch { condition, no, .. } = block.terminator else {
                    continue;
                };
                let primary = matches!(
                    body.typed()
                        .body()
                        .expressions()
                        .get(condition)
                        .unwrap()
                        .kind,
                    ExpressionKind::PrimaryCondition { .. }
                );
                let expected = if primary && index == 0 {
                    flow::Terminator::Jump(no)
                } else {
                    block.terminator
                };
                assert_eq!(
                    control.terminator(point),
                    Some(expected),
                    "{name}, frame {index}, primary {primary}"
                );
                branches += 1;
            }
            assert_eq!(branches, 2);
        }
    }
}
