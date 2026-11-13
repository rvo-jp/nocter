//! Source-flow recipes survive result code/type projection in the exact accepted Unit.
use super::*;
use crate::value_access::{ExpressionKind, ProducerRelation, ProducerTerm};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{
    AccessCodeId, AccessExpressionId, ValueApplication, ValueFunctionOrigin as Origin,
};
use std::collections::BTreeSet;

fn code(unit: &Unit, name: &str) -> AccessCodeId {
    unit.program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap()
}
fn call(unit: &Unit, owner: &str, callee: &str) -> AccessExpressionId {
    let target = Origin::Declaration(code(unit, callee));
    named(unit, owner).typed().body().expressions().iter().find_map(|(id, expression)| {
        matches!(&expression.kind, ExpressionKind::Call { target: selected, .. } if *selected == target).then_some(id)
    }).unwrap()
}
fn application(unit: &Unit, owner: &str, call: AccessExpressionId) -> ProducerTerm {
    ProducerTerm::Application(ValueApplication {
        owner: code(unit, owner).into(),
        call,
    })
}

#[test]
fn distinct_applications_keep_inputs_even_when_their_result_types_project_to_one_closure() {
    let unit = accepted(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc make(action: func(): i32): func(): i32 { let n = 42\n(n;): i32 { n } }\nfunc run(): i32 { let left = make(first)\nlet right = make(second)\nleft() + right() }",
    );
    let maker = code(&unit, "make");
    let parameter = unit.program().codes().get(maker).unwrap().parameters()[0].into();
    let body = named(&unit, "run").typed().body();
    let calls = body.expressions().iter().filter_map(|(id, expression)| {
        matches!(&expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if *target == maker).then_some((id, expression.ty.access().unwrap().data()))
    }).collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].1, calls[1].1);
    for ((id, _), name) in calls.iter().zip(["first", "second"]) {
        let Some(ProducerRelation::Apply { callee, inputs }) =
            unit.producer_relation(application(&unit, "run", *id))
        else {
            panic!("original application recipe");
        };
        assert_eq!(*callee, maker.into());
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs[&parameter],
            BTreeSet::from([FunctionWitness::from(code(&unit, name))])
        );
    }
    let Some(ProducerRelation::Union(returned)) =
        unit.producer_relation(ProducerTerm::Result(maker.into()))
    else {
        panic!("source return recipe");
    };
    assert_eq!(
        returned,
        &BTreeSet::from([unit.result_witness(maker).unwrap()])
    );
}

const FORWARDED: &str = "func first(): i32 { 42 }\nfunc make(): func(): i32 { first }\nfunc forward(take action: func(): i32): func(): i32 { action^ }\nfunc run(): i32 { let action = forward(make())\naction() }";

#[test]
fn forwarded_application_keeps_its_original_producer_term_after_selected_code_projection() {
    let unit = accepted(FORWARDED);
    let inner = call(&unit, "run", "make");
    let outer = call(&unit, "run", "forward");
    let forward = code(&unit, "forward");
    let parameter = unit.program().codes().get(forward).unwrap().parameters()[0].into();
    let Some(ProducerRelation::Apply { inputs, .. }) =
        unit.producer_relation(application(&unit, "run", outer))
    else {
        panic!("forwarded application");
    };
    let original = inputs[&parameter].first().unwrap();
    assert_eq!(inputs[&parameter].len(), 1);
    let Origin::Application(origin) = original.origin else {
        panic!("original nested application");
    };
    assert_eq!(origin.call, inner);
    assert_eq!(origin.owner, code(&unit, "run").into());
    assert!(
        matches!(unit.producer_relation(ProducerTerm::Application(origin)), Some(ProducerRelation::Apply { callee, .. }) if callee.origin == Origin::Declaration(code(&unit, "make")))
    );
    let expression = named(&unit, "run")
        .typed()
        .body()
        .expressions()
        .get(outer)
        .unwrap();
    let ExpressionKind::Call { arguments, .. } = &expression.kind else {
        panic!("projected call");
    };
    assert_eq!(
        arguments[0].binding.as_ref().unwrap().witness.origin,
        Origin::Declaration(code(&unit, "first"))
    );
}

#[test]
fn producer_recipes_remain_paired_with_the_accepted_units_body_sources() {
    let (sources, tree) = input(FORWARDED);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    unit.bind(&context).unwrap();
    let changed = FORWARDED.replace("forward(make())", "forward(first)");
    let (changed_sources, changed_tree) = input(&changed);
    let current = headers
        .bind(bound(&changed_sources, &changed_tree))
        .unwrap();
    assert!(matches!(
        unit.bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    let rebound = Unit::prepare(&current).unwrap();
    let outer = call(&rebound, "run", "forward");
    let Some(ProducerRelation::Apply { inputs, .. }) =
        rebound.producer_relation(application(&rebound, "run", outer))
    else {
        panic!("current recipe");
    };
    assert!(
        inputs
            .values()
            .flatten()
            .all(|witness| witness.origin == Origin::Declaration(code(&rebound, "first")))
    );
}
