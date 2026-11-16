//! Actual prepared values, including synthesized construction, own invocation payload selection.
use super::*;

#[test]
fn compound_updates_and_implicit_outcome_construction_retain_prepared_value_evidence() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/invocation-sources.nct"
    )));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let calculate = callable(graph.unit(), "calculate");
    let (_, caller) = graph
        .instances()
        .iter()
        .find(|(_, instance)| instance.key().code() == ValueCode::Declaration(calculate))
        .unwrap();
    assert_eq!(caller.calls().len(), 1);
    let call = caller.calls().values().next().unwrap();
    let argument = &call.inputs()[0].payload;
    assert!(matches!(
        graph.unit().program().data_schema(argument.data()),
        Some(nocter_declarations::value_access::PayloadSchema::Optional(
            _
        ))
    ));
    let source = graph.unit().value_sources(caller.key().code()).unwrap();
    let prepared = source.call(*caller.calls().keys().next().unwrap()).unwrap()[0].arguments[0];
    assert!(has_primary_construction(source, prepared));
}

fn has_primary_construction(
    graph: &nocter_checking::value_access::ValueSources,
    root: nocter_checking::value_access::ValueSourceId,
) -> bool {
    use nocter_checking::value_access::ValueSource;
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let value = &graph.node(id).unwrap().value;
        match value {
            ValueSource::Aggregate(fields)
                if fields
                    .iter()
                    .any(|(field, _)| *field == nocter_model::ValueField::Variant(true)) =>
            {
                return true;
            }
            ValueSource::Adapt { source } => pending.push(*source),
            ValueSource::Merge { .. } | ValueSource::Alternatives(_) => {
                pending.extend(value.inputs());
            }
            _ => {}
        }
    }
    false
}

#[test]
fn positive_domain_calls_close_every_candidate_channel_before_execution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let fixture = ValueSourceFixture::new(&format!(
        "{definitions}\nfunc run(): i32 {{ calculate(true) }}"
    ));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let calls: Vec<_> = graph
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.calls().values())
        .filter(|call| call.target() == CallTarget::Selected)
        .collect();
    assert!(!calls.is_empty());
    for call in calls {
        assert_eq!(call.cases().len(), 2);
        assert_ne!(call.cases()[0].callee, call.cases()[1].callee);
        let expected = call
            .argument_payload(0)
            .unwrap()
            .instance()
            .function_representations(graph.unit().program())
            .unwrap()
            .unwrap();
        assert_eq!(
            expected.as_ref(),
            call.cases()
                .iter()
                .map(|case| case.callee.clone())
                .collect::<Vec<_>>()
        );
        for case in call.cases() {
            let CallTarget::Body(target) = case.target else {
                panic!("expected body");
            };
            let frame = graph.instances().get(target).unwrap().key();
            let nocter_declarations::value_access::InstanceSchema::Function(witness) =
                case.callee.schema(graph.unit().program()).unwrap()
            else {
                panic!("complete callable case");
            };
            assert_eq!(frame.code(), witness.code());
            assert_eq!(frame.environment(), witness.environment());
        }
    }
    assert_domain_binding_retry(&graph);
}

fn assert_domain_binding_retry(graph: &InstanceProgram) {
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    let (owner, call, expected) = graph
        .instances()
        .iter()
        .find_map(|(_, instance)| {
            instance
                .calls()
                .iter()
                .find(|(_, call)| call.target() == CallTarget::Selected)
                .map(|(call, edge)| (instance, *call, edge))
        })
        .unwrap();
    let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    let unit = Arc::new(graph.unit().clone());
    let queries = unit.instance_metadata(owner.key(), &mut budget()).unwrap();
    let site = nocter_model::ValueApplication {
        owner: owner.key().code(),
        call,
    };
    let domain = queries.invocation_domain(site, &mut budget()).unwrap();
    let caller = unit.code_body(site.owner).unwrap();
    let target = |frame: &InstanceKey| {
        CallTarget::Body(
            graph
                .instances()
                .iter()
                .find(|(_, instance)| instance.key() == frame)
                .unwrap()
                .0,
        )
    };
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    assert!(matches!(
        invocation::bind_domain(&queries, caller, &domain, target, &mut exhausted),
        Err(Failure::Invocation {
            failure: nocter_checking::value_access::InstantiationFailure::WorkLimit,
            ..
        })
    ));
    assert_eq!(
        &invocation::bind_domain(&queries, caller, &domain, target, &mut budget()).unwrap(),
        expected
    );
}

#[test]
fn explicit_callable_moves_keep_the_declared_read_invocation_channels() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    assert!(definitions.contains("callback(if present"));
    let definitions = definitions.replace("callback(if present", "callback^(if present");
    let fixture = ValueSourceFixture::new(&format!(
        "{definitions}\nfunc run(): i32 {{ calculate(true) }}"
    ));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let call = graph
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.calls().values())
        .find(|call| call.target() == CallTarget::Selected)
        .unwrap();
    assert_eq!(call.cases().len(), 2);
    assert!(call.cases().iter().all(|case| matches!(
        case.environment,
        Environment::Capture {
            supplied: nocter_model::ValueInputOperation::Read,
            exit: EnvironmentExit::Retained,
            ..
        }
    )));
}

#[test]
fn finite_callable_candidates_do_not_admit_unqualified_once_invocations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst func calculate").unwrap();
    let definitions = definitions
        .replace("func(i32): i32", "once(i32): i32")
        .replace("callback(amount)", "callback^(amount)");
    let fixture = ValueSourceFixture::new(&format!(
        "{definitions}\nfunc evaluate(present: bool): i32 {{ let value = maybe(present)\nlet callback = value^ otherwise {{ return 0 }}\ncallback^(42) }}\nfunc run(): i32 {{ evaluate(true) }}"
    ));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    match InstanceProgram::prepare(unit, root) {
        Err(failure) => assert_eq!(failure, Failure::InvalidAcceptedUnit),
        Ok(_) => panic!("consuming candidate dispatch remains unqualified"),
    }
}
