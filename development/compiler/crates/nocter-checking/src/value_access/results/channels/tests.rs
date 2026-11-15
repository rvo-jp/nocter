//! Real source boundaries retain canonical inputs without granting an executable body.
use super::*;
use crate::value_access::{CallKind, ExpressionKind, accepted::SourceUnit};
use nocter_constant_evaluation::state::{Limits, WorkBudget};
use nocter_model::{ValueApplication, ValueFunctionOrigin};
use nocter_test_support::ValueSourceFixture;
use std::num::NonZeroU64;

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
