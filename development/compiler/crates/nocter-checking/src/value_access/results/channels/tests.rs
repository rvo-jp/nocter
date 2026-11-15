//! Real source boundaries retain canonical inputs without granting an executable body.
use super::*;
use crate::value_access::{CallKind, ExpressionKind, accepted::SourceUnit};
use nocter_constant_evaluation::state::{Limits, WorkBudget};
use nocter_model::{ValueApplication, ValueFunctionOrigin};
use nocter_test_support::ValueSourceFixture;
use std::num::NonZeroU64;

#[test]
fn captured_metadata_boundaries_preserve_environment_modes_and_explicit_context_ordinals() {
    let source = ValueSourceFixture::new(
        "struct Item { offset: i32 }\nfunc leaf(value: i32): i32 { value + 7 }\nfunc root(): i32 { let number = 7\nlet item = Item { offset: 7 }\nlet reading = (number; take value: (func(i32): i32)?): i32 { let action = value^ otherwise { return 42 }\naction(35) }\nvar editing = (number; take value: (func(i32): i32)?): i32 { number += 0\nlet action = value^ otherwise { return 42 }\naction(35) }\nlet taking = (item^; take value: (func(i32): i32)?): i32 { let owned = item^\nlet action = value^ otherwise { return owned.offset + 35 }\naction(35) }\nreading(leaf) + editing(leaf) + taking^(leaf) - 84 }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let root = program
        .bind_code(
            program
                .resolve(program.symbols().get("root").unwrap())
                .unwrap()
                .into(),
            [],
        )
        .unwrap();
    let body = unit.code_body(root.code()).unwrap();
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    let healthy = || WorkBudget::new(Limits::default().steps());
    let mut modes = std::collections::BTreeSet::new();
    for (call, expression) in body.typed().body().expressions().iter() {
        if !matches!(
            expression.kind,
            ExpressionKind::Call {
                target: ValueFunctionOrigin::Closure(_),
                ..
            }
        ) {
            continue;
        }
        let frame = unit
            .instantiate_invocation(
                &root,
                ValueApplication {
                    owner: root.code(),
                    call,
                },
                &mut healthy(),
            )
            .unwrap();
        let original = boundary(
            program,
            evidence,
            unit.code_body(frame.code()),
            &frame,
            &mut healthy(),
        )
        .unwrap();
        let linked = boundary(program, evidence, None, &frame, &mut healthy()).unwrap();
        assert_eq!(linked.inputs, original.inputs);
        assert_eq!(linked.result, original.result);
        assert_eq!(linked.environment_backing, original.environment_backing);
        assert_eq!(linked.inputs.len(), 2);
        assert_eq!(Some(&linked.inputs[1].data), frame.input_data(0));
        assert_eq!(
            Some(linked.inputs[0].data.clone()),
            program.instantiate_environment(&frame).unwrap()
        );
        modes.insert(linked.inputs[0].operation);
        for quota in [1, 2] {
            assert!(matches!(
                boundary(
                    program,
                    evidence,
                    None,
                    &frame,
                    &mut WorkBudget::new(NonZeroU64::new(quota).unwrap())
                ),
                Err(InstantiationFailure::WorkLimit)
            ));
        }
        assert_eq!(
            boundary(program, evidence, None, &frame, &mut healthy())
                .unwrap()
                .inputs,
            linked.inputs
        );
    }
    assert_eq!(
        modes,
        std::collections::BTreeSet::from([
            nocter_model::ValueInputOperation::Read,
            nocter_model::ValueInputOperation::Edit,
            nocter_model::ValueInputOperation::Take
        ])
    );
}

#[test]
fn declaration_only_boundaries_match_body_channels_and_preserve_received_contexts() {
    let source = ValueSourceFixture::new(
        "func leaf(value: i32): i32 { value }\nfunc plain(take value: i32, edit number: i32): i32 { value + number }\nfunc accept(action: func(i32): i32, take optional: (func(i32): i32)?, edit number: i32): void {}\nfunc root(): void { var number = 1\nlet _ = plain(1, number)\naccept(leaf, leaf, number) }\nfunc factory(): func(i32): i32 { leaf }",
    );
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let healthy = || WorkBudget::new(Limits::default().steps());
    let root = program.bind_code(code("root").into(), []).unwrap();
    let body = unit.code_body(root.code()).unwrap();
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    for name in ["plain", "accept"] {
        let call = body.typed().body().expressions().iter().find_map(|(call, expression)|
            matches!(expression.kind, ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), kind: CallKind::Direct, .. }
                if target == code(name))
                .then_some(ValueApplication { owner: root.code(), call })).unwrap();
        let frame = unit
            .instantiate_invocation(&root, call, &mut healthy())
            .unwrap();
        let selected = unit.code_body(frame.code()).unwrap();
        let original = boundary(program, evidence, Some(selected), &frame, &mut healthy()).unwrap();
        let without_body = boundary(program, evidence, None, &frame, &mut healthy()).unwrap();
        assert_eq!(without_body.inputs, original.inputs);
        assert_eq!(without_body.result, original.result);
        assert!(!without_body.environment_backing);
        if name == "accept" {
            assert_eq!(Some(&without_body.inputs[1].data), frame.input_data(1));
            assert!(matches!(
                without_body.inputs[0].data.schema(program).unwrap(),
                nocter_declarations::value_access::InstanceSchema::Function(_)
            ));
        }
        assert!(matches!(
            boundary(program, evidence, Some(body), &frame, &mut healthy()),
            Err(InstantiationFailure::WrongOwner)
        ));
        assert!(matches!(
            boundary(
                program,
                evidence,
                None,
                &frame,
                &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
            ),
            Err(InstantiationFailure::WorkLimit)
        ));
        assert_eq!(
            boundary(program, evidence, None, &frame, &mut healthy())
                .unwrap()
                .inputs,
            without_body.inputs
        );
    }
    let factory = program.bind_code(code("factory").into(), []).unwrap();
    assert!(
        matches!(
            boundary(program, evidence, None, &factory, &mut healthy()),
            Err(InstantiationFailure::Unresolved)
        ),
        "result producers require their original body evidence"
    );
    assert!(
        boundary(
            program,
            evidence,
            unit.code_body(factory.code()),
            &factory,
            &mut healthy()
        )
        .is_ok()
    );
}
