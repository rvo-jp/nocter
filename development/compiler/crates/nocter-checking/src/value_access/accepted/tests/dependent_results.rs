//! Actual witness substitution, canonical authority and generic ownership through returned callbacks.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{ArgumentAction, CallKind, ExpressionKind, Query as TypedQuery};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::ValueFunctionOrigin as Origin;

#[test]
fn separate_applications_retain_distinct_witnesses_in_one_canonical_unit() {
    let (sources, tree) = input(&source_case("a123-distinct-result-applications.nct"));
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let run = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let raw = TypedQuery::prepare(&context, run).unwrap();
    let applications = raw
        .body()
        .locals()
        .iter()
        .filter_map(|(_, local)| match raw.program().data_schema(local.data) {
            Some(PayloadSchema::Function(w)) if matches!(w.origin, Origin::Application(_)) => {
                Some(w)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2);
    assert_ne!(applications[0].origin, applications[1].origin);
    assert!(
        applications
            .iter()
            .all(|&w| context.program().function_data(w).is_none())
    );
    let unit = Unit::prepare(&context).unwrap();
    for (_, body) in unit.bodies() {
        assert!(std::ptr::eq(body.typed().program(), unit.program()));
        for (_, expression) in body.typed().body().expressions().iter() {
            if let Some(PayloadSchema::Function(w)) = expression
                .ty
                .access()
                .and_then(|a| unit.program().data_schema(a.data()))
            {
                assert!(!matches!(
                    w.origin,
                    Origin::Result(_) | Origin::Application(_)
                ));
            }
        }
    }
    let data = named(&unit, "run")
        .typed()
        .body()
        .locals()
        .iter()
        .map(|(_, local)| local.data)
        .collect::<Vec<_>>();
    assert_eq!(data.len(), 2);
    assert_ne!(data[0], data[1]);
    for ty in data {
        assert_eq!(unit.program().data_copyable(ty), Some(true));
    }
    assert!(unit.bind(&context).is_ok());
}

#[test]
fn generic_returned_environments_keep_transfer_and_symbolic_cleanup() {
    let unit = accepted(&source_case("a129-dependent-result-once-transfer.nct"));
    let relay = named(&unit, "relay");
    let witness = unit.result_witness(relay.typed().callable()).unwrap();
    let Origin::Parameter(parameter) = witness.origin else {
        panic!("formal result")
    };
    assert_eq!(
        unit.program().parameters().get(parameter).unwrap().owner(),
        relay.typed().callable()
    );
    assert_eq!(
        unit.program().data_copyable(relay.typed().result_data()),
        Some(false)
    );
    let invoke = named(&unit, "invoke");
    let consuming = invoke
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            if let ExpressionKind::Call {
                kind: CallKind::Value,
                arguments,
                ..
            } = &expression.kind
            {
                Some(&arguments[0])
            } else {
                None
            }
        })
        .unwrap();
    assert!(matches!(
        consuming.action,
        ArgumentAction::ReserveTransfer(_)
    ));
    assert!(steps(invoke).iter().any(|step| matches!(step, Step::Receive { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)));
    assert!(steps(relay).iter().any(|step| matches!(step, Step::Move { destination, payload, .. } if destination.holder() == Holder::Return && payload.disposal() == Disposal::FunctionEnvironment)));
}

#[test]
fn mutual_result_relations_substitute_the_selected_formal_positions() {
    let unit = accepted(&source_case("a126-dependent-result-permutation.nct"));
    for (name, parameter) in [("first", "a"), ("second", "y")] {
        let body = named(&unit, name);
        let witness = unit.result_witness(body.typed().callable()).unwrap();
        let Origin::Parameter(id) = witness.origin else {
            panic!("formal result")
        };
        let declaration = unit.program().parameters().get(id).unwrap();
        assert_eq!(declaration.owner(), body.typed().callable());
        assert_eq!(
            unit.program().symbols().spelling(declaration.name()),
            Some(parameter)
        );
    }
}

#[test]
fn returning_the_second_formal_stages_it_without_disposing_it_with_the_first() {
    let unit = accepted(&source_case("a134-dependent-result-second-input.nct"));
    let query = named(&unit, "choose");
    let result = query.typed().result_data();
    let trace = steps(query);
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)).count(), 1);
    assert!(trace.iter().any(|step| matches!(step, Step::Move { destination, payload, .. } if destination.holder() == Holder::Return && payload.data() == result)));
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Dispose { payload, .. } if payload.data() == result))
    );
}

#[test]
fn interrupted_application_keeps_its_known_witness_without_producing_an_environment() {
    let unit = accepted(&source_case("a131-dependent-result-interruption.nct"));
    let query = named(&unit, "run");
    assert!(
        query
            .typed()
            .body()
            .expressions()
            .iter()
            .any(|(_, e)| !e.ty.may_complete()
                && e.ty.access().is_some_and(|a| matches!(
                    unit.program().data_schema(a.data()),
                    Some(PayloadSchema::Function(_))
                )))
    );
    assert!(
        !steps(query)
            .iter()
            .any(|step| matches!(step, Step::Receive { .. } | Step::Invoke { .. }))
    );
}
