use super::*;
use nocter_checking::value_access::CallKind;
use nocter_declarations::value_access::FunctionWitness;

#[test]
fn executable_selection_rejects_an_unbound_or_weaker_view_even_when_its_underlying_code_can_read() {
    let fixture = ValueSourceFixture::new(
        "func identity(value: i32): i32 { value }\nfunc apply(action: func(i32): i32): i32 { action(42) }\nfunc run(): i32 { apply(identity) }",
    );
    let unit = accepted(&fixture);
    let apply = callable(&unit, "apply");
    let identity = callable(&unit, "identity");
    let parameter = unit.program().codes().get(apply).unwrap().parameters()[0].into();
    let body = unit.body(apply).unwrap().typed().body();
    let call = body
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(&expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .unwrap();
    assert!(
        unit.program()
            .bind_code(ValueCode::Declaration(apply), [])
            .is_err()
    );
    assert!(
        unit.program()
            .bind_code(
                ValueCode::Declaration(apply),
                [(
                    parameter,
                    FunctionWitness {
                        origin: ValueFunctionOrigin::Declaration(identity),
                        invocation: ValueInvocation::Edit,
                    }
                )]
            )
            .is_err()
    );
    let caller = unit
        .program()
        .bind_code(
            ValueCode::Declaration(apply),
            [(parameter, identity.into())],
        )
        .unwrap();
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        std::num::NonZeroU64::new(10_000).unwrap(),
    );
    let selected = unit
        .instantiate_invocation(
            &caller,
            nocter_model::ValueApplication {
                owner: caller.code(),
                call,
            },
            &mut work,
        )
        .unwrap();
    assert_eq!(selected.code(), ValueCode::Declaration(identity));
    assert!(selected.witnesses().is_empty());
    assert_eq!(
        caller
            .resolve(
                unit.program(),
                FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(parameter),
                    invocation: ValueInvocation::Edit
                }
            )
            .unwrap()
            .invocation(),
        ValueInvocation::Edit
    );
}

#[test]
fn explicit_edit_inputs_cannot_reclassify_a_direct_call_as_an_editable_environment() {
    let fixture = ValueSourceFixture::new(
        "func update(edit value: i32): void { value += 1 }\nfunc run(): i32 { var value = 41\nupdate(value)\nvalue }",
    );
    let unit = accepted(&fixture);
    let run = callable(&unit, "run");
    let (kind, arguments) = unit
        .body(run)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                kind, arguments, ..
            } => Some((*kind, arguments.as_ref())),
            _ => None,
        })
        .unwrap();
    assert_eq!(kind, CallKind::Direct);
    assert_eq!(
        arguments[0].operation,
        nocter_model::ValueInputOperation::Edit
    );
    assert_eq!(kind.invocation(arguments), Some(ValueInvocation::Read));
    assert_eq!(CallKind::Value.invocation(&[]), None);
    assert_eq!(
        CallKind::Direct.invocation(&[]),
        Some(ValueInvocation::Read)
    );
}

#[test]
fn reached_invocations_share_one_work_quota_and_failure_does_not_publish_a_partial_graph() {
    use nocter_checking::value_access::InstantiationFailure;
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    let unit = accepted(&ValueSourceFixture::new(
        "func leaf(): void {}\nfunc run(): void { leaf()\nleaf() }",
    ));
    let root = callable(&unit, "run");
    let caller = unit.program().bind_code(root.into(), []).unwrap();
    let calls = unit
        .body(root)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(id, expression)| {
            matches!(&expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    let second = nocter_model::ValueApplication {
        owner: caller.code(),
        call: calls[1],
    };
    // Find the observable boundary after the first commit. Earlier continuation/preparation
    // work also spends this quota; no fixed cost or per-invocation replenishment is assumed.
    let minimum = (1..=1_024)
        .find(|&units| {
            let mut work = WorkBudget::new(NonZeroU64::new(units).unwrap());
            matches!(InstanceProgram::prepare_with_budget(unit.clone(), root, &mut work), Err(Failure::Invocation { site, failure: InstantiationFailure::WorkLimit }) if site == second)
        })
        .expect("both reached invocations share a bounded metadata quota");
    let mut work = WorkBudget::new(NonZeroU64::new(minimum).unwrap());
    let failure = InstanceProgram::prepare_with_budget(unit.clone(), root, &mut work).unwrap_err();
    assert_eq!(
        failure,
        Failure::Invocation {
            site: second,
            failure: InstantiationFailure::WorkLimit,
        }
    );
    let graph = InstanceProgram::prepare(unit.clone(), root).unwrap();
    assert_eq!(graph.instances().len(), 2);
    assert_eq!(
        graph.instances().get(graph.root()).unwrap().calls().len(),
        2
    );
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    assert!(matches!(
        InstanceProgram::prepare_with_budget(unit, root, &mut exhausted),
        Err(Failure::Control {
            failure: InstantiationFailure::WorkLimit,
            ..
        })
    ));
}
