//! Structural checks survive interruption; no absent callback acquires a payload or live use.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{ArgumentAction, BoundaryAction, ExpressionKind, ValueType};
use nocter_model::{ValueAccess, ValueAccessMode};

const FORWARD: &str = "func forward(take action: func(i32): i32): func(i32): i32 { action^ }\n";

fn absent(unit: &Unit, name: &str) -> Vec<nocter_model::TypeId> {
    let query = named(unit, name);
    let mut data = std::collections::BTreeSet::new();
    for (_, expression) in query.typed().body().expressions().iter() {
        if let Some(access) = expression.ty.access()
            && matches!(
                unit.data_evidence(access.data()).unwrap(),
                DataEvidence::Unproduced
            )
        {
            data.insert(access.data());
            assert!(flow::resources::payload(unit.program(), access.data()).is_err());
        }
    }
    assert!(!data.is_empty());
    for step in steps(query) {
        step.try_map_payloads::<(), _>(|payload| {
            assert!(!data.contains(&payload.data()));
            Ok(*payload)
        })
        .unwrap();
    }
    data.into_iter().collect()
}

#[test]
fn interrupted_structures_keep_shapes_without_payloads_and_clean_only_completed_components() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/structural-result-values.nct"
    )));
    let data = absent(&unit, "interrupted");
    assert!(data.iter().any(|&data| matches!(
        unit.program().data_schema(data),
        Some(nocter_declarations::value_access::PayloadSchema::Array(_))
    )));
    assert!(data.iter().any(|&data| matches!(
        unit.program().data_schema(data),
        Some(nocter_declarations::value_access::PayloadSchema::Aggregate(
            _
        ))
    )));
    absent(&unit, "interrupted_components");
    let trace = steps(named(&unit, "interrupted_components"));
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedCapture)).count(), 1);
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Assemble { .. }))
    );
}

#[test]
fn interrupted_unknown_input_retains_a_contract_without_a_payload_or_invocation() {
    let unit = accepted(&source_case("a137-dependent-result-unknown-input.nct"));
    absent(&unit, "invalid");
    assert!(
        !steps(named(&unit, "invalid"))
            .iter()
            .any(|step| matches!(step, Step::Invoke { .. } | Step::Receive { .. }))
    );
}

#[test]
fn unreachable_locals_aliases_adapters_and_pending_supplies_do_not_construct_environments() {
    let unit = accepted(&format!(
        "{FORWARD}
        func consume(take action: once(i32): i32): void {{}}
        func run(): i32 {{
            var missing = forward(if true {{ return 1 }} else {{ return 2 }})
            let next = missing
            read alias = missing
            let adapted: once(i32): i32 = missing
            missing = next
            consume(adapted)
            return alias(1)
        }}"
    ));
    absent(&unit, "run");
    let expressions = named(&unit, "run").typed().body().expressions();
    assert!(expressions.iter().any(|(_, e)| matches!(
        e.kind,
        ExpressionKind::Boundary {
            action: BoundaryAction::Unproduced,
            ..
        }
    )));
    assert!(expressions.iter().any(|(_, e)| matches!(&e.kind,
        ExpressionKind::Call { arguments, .. }
        if arguments.iter().any(|a| a.action == ArgumentAction::Unproduced))));
}

#[test]
fn nested_dependent_applications_preserve_absence_through_forwarding() {
    let unit = accepted(&format!(
        "{FORWARD}
        func run(): i32 {{
            forward(forward(if true {{ return 1 }} else {{ return 2 }}))(1)
        }}"
    ));
    assert_eq!(absent(&unit, "run").len(), 2);
}

#[test]
fn recursive_return_anchors_come_from_real_returns_not_absent_arguments() {
    let unit = accepted(&format!(
        "{FORWARD}
        func increment(value: i32): i32 {{ value + 1 }}
        func factory(flag: bool): func(i32): i32 {{
            if flag {{
                forward(if true {{ return factory(false) }} else {{ return factory(false) }})
            }} else {{ increment }}
        }}"
    ));
    absent(&unit, "factory");
    let factory = named(&unit, "factory").typed().source_owner();
    let increment = named(&unit, "increment").typed().source_owner();
    assert_eq!(
        unit.result_witness(factory).unwrap().origin,
        nocter_model::ValueFunctionOrigin::Declaration(increment)
    );
}

#[test]
fn interrupted_calls_still_reject_wrong_arity_data_and_annotation_at_the_source() {
    for tail in [
        "forward(if true { return 1 } else { return 2 })(true)",
        "forward(if true { return 1 } else { return 2 })()",
        "let value: func(bool): i32 = forward(if true { return 1 } else { return 2 })\n return 0",
    ] {
        let source = format!("{FORWARD}func run(): i32 {{ {tail} }}");
        let (sources, tree) = input(&source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let Err(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        }) = Unit::prepare(&context)
        else {
            panic!("expected source-backed rejection: {tail}")
        };
        assert!(!diagnostic.source().primary().span().range().is_empty());
    }
}

#[test]
fn signature_only_evidence_cannot_be_used_as_a_reachable_runtime_value() {
    let unit = accepted(&source_case("a137-dependent-result-unknown-input.nct"));
    let data = absent(&unit, "invalid")[0];
    let mut typed = named(&unit, "invalid").typed().clone();
    typed.body.expressions = typed.body.expressions.map(|_, expression| {
        let mut expression = expression.clone();
        if matches!(expression.kind, ExpressionKind::Integer(_)) {
            expression.ty = ValueType::known(ValueAccess::new(data, ValueAccessMode::Own));
        }
        expression
    });
    assert!(matches!(
        flow::Query::prepare(Arc::new(typed)),
        Err(flow::IntegrityError::InconsistentTypedBody)
    ));
}
