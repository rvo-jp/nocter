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
        let Some(ProducerRelation::Apply(invocation)) =
            unit.producer_relation(application(&unit, "run", *id))
        else {
            panic!("original application recipe");
        };
        let recipe = unit
            .invocation_recipe(ValueApplication {
                owner: code(&unit, "run").into(),
                call: *id,
            })
            .unwrap();
        assert!(
            std::ptr::eq(invocation.as_ref(), recipe),
            "one shared invocation record"
        );
        assert_eq!(invocation.callees, BTreeSet::from([maker.into()]));
        assert_eq!(invocation.inputs.len(), 1);
        assert_eq!(
            invocation.inputs[&parameter],
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
fn equal_typed_tuple_fields_keep_their_own_producer_occurrences_after_reassignment() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/component-producers.nct"
    )));
    for (owner, expected) in [
        ("snapshot", [vec![1], vec![0]]),
        ("replace_component", [vec![2], vec![0]]),
        ("choose_component", [vec![1, 2], vec![0, 3]]),
        ("repeated_components", [vec![1], vec![0]]),
        ("summarized_elements", [vec![0, 1], vec![0, 1]]),
    ] {
        assert_component_producers(&unit, owner, &expected);
    }
    let answer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}

fn assert_component_producers(unit: &Unit, name: &str, expected: &[Vec<usize>]) {
    let owner = code(unit, name).into();
    let maker = Origin::Declaration(code(unit, "make"));
    let body = named(unit, name).typed().body();
    let mut creations = Vec::new();
    let mut uses = Vec::new();
    for (call, expression) in body.expressions().iter() {
        if let ExpressionKind::Call { target, kind, .. } = &expression.kind {
            if *target == maker {
                creations.push(ValueApplication { owner, call });
            } else if *kind == crate::value_access::CallKind::Value {
                uses.push(ValueApplication { owner, call });
            }
        }
    }
    assert_eq!(uses.len(), expected.len(), "{name}");
    for (site, alternatives) in uses.into_iter().zip(expected) {
        assert_eq!(
            unit.invocation_recipe(site).unwrap().callees,
            alternatives
                .iter()
                .map(|&index| FunctionWitness {
                    origin: Origin::Application(creations[index]),
                    invocation: nocter_model::ValueInvocation::Read,
                })
                .collect::<BTreeSet<_>>(),
            "{name}",
        );
    }
}

#[test]
fn forwarded_application_keeps_its_original_producer_term_after_selected_code_projection() {
    let unit = accepted(FORWARDED);
    let inner = call(&unit, "run", "make");
    let outer = call(&unit, "run", "forward");
    let forward = code(&unit, "forward");
    let parameter = unit.program().codes().get(forward).unwrap().parameters()[0].into();
    let Some(ProducerRelation::Apply(invocation)) =
        unit.producer_relation(application(&unit, "run", outer))
    else {
        panic!("forwarded application");
    };
    let original = invocation.inputs[&parameter].first().unwrap();
    assert_eq!(invocation.inputs[&parameter].len(), 1);
    let Origin::Application(origin) = original.origin else {
        panic!("original nested application");
    };
    assert_eq!(origin.call, inner);
    assert_eq!(origin.owner, code(&unit, "run").into());
    assert!(
        matches!(unit.producer_relation(ProducerTerm::Application(origin)), Some(ProducerRelation::Apply(recipe)) if recipe.callees == BTreeSet::from([FunctionWitness::from(code(&unit, "make"))]))
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
    let Some(ProducerRelation::Apply(invocation)) =
        rebound.producer_relation(application(&rebound, "run", outer))
    else {
        panic!("current recipe");
    };
    assert!(
        invocation
            .inputs
            .values()
            .flatten()
            .all(|witness| witness.origin == Origin::Declaration(code(&rebound, "first")))
    );
}

#[test]
fn scalar_result_calls_retain_original_callable_argument_and_receiver_producers() {
    let unit = accepted(
        "func first(): i32 { 42 }\nfunc make(): func(): i32 { first }\nfunc consume(action: func(): i32): i32 { action() }\nfunc run(): i32 { consume(make()) }",
    );
    let maker = call(&unit, "run", "make");
    let consumer = call(&unit, "run", "consume");
    let site = ValueApplication {
        owner: code(&unit, "run").into(),
        call: consumer,
    };
    assert!(
        unit.producer_relation(ProducerTerm::Application(site))
            .is_none()
    );
    let recipe = unit.invocation_recipe(site).unwrap();
    assert_eq!(
        recipe.callees,
        BTreeSet::from([FunctionWitness::from(code(&unit, "consume"))])
    );
    let actual = recipe.inputs.values().flatten().collect::<Vec<_>>();
    assert_eq!(actual.len(), 1);
    assert!(
        matches!(actual[0].origin, Origin::Application(origin) if origin.owner == site.owner && origin.call == maker)
    );
    let body = named(&unit, "run").typed().body();
    let ExpressionKind::Call { arguments, .. } = &body.expressions().get(consumer).unwrap().kind
    else {
        panic!("projected scalar call");
    };
    assert_eq!(
        arguments[0].binding.as_ref().unwrap().witness.origin,
        Origin::Declaration(code(&unit, "first"))
    );

    let direct_value = accepted(
        "func first(): i32 { 42 }\nfunc make(): func(): i32 { first }\nfunc run(): i32 { let action = make()\naction() }",
    );
    let inner = call(&direct_value, "run", "make");
    let invocation = call(&direct_value, "run", "first");
    let site = ValueApplication {
        owner: code(&direct_value, "run").into(),
        call: invocation,
    };
    let recipe = direct_value.invocation_recipe(site).unwrap();
    assert!(
        matches!(recipe.callees.first().unwrap().origin, Origin::Application(origin) if origin.call == inner && origin.owner == site.owner)
    );
}

#[test]
fn an_interrupted_call_has_neither_a_committed_invocation_recipe_nor_a_producer_anchor() {
    let source = FORWARDED.replace(
        "forward(make())",
        "forward(if true { return 42 } else { return 0 })",
    );
    let unit = accepted(&source);
    let outer = call(&unit, "run", "forward");
    let site = ValueApplication {
        owner: code(&unit, "run").into(),
        call: outer,
    };
    assert!(unit.invocation_recipe(site).is_none());
    assert!(unit.producer_relation(ProducerTerm::Application(site)).is_none_or(|relation| matches!(relation, ProducerRelation::Union(producers) if producers.is_empty())));
}

#[test]
fn receiver_recipes_follow_actual_reassignment_and_preserve_control_alternatives() {
    for (replacement, alternatives) in [
        ("action = make(second)", 1),
        ("if choose { action = make(second) }", 2),
    ] {
        let source = format!(
            "func first(): i32 {{ 41 }}\nfunc second(): i32 {{ 42 }}\nfunc make(input: func(): i32): func(): i32 {{ let n = 42\n(n;): i32 {{ n }} }}\nfunc run(choose: bool): i32 {{ var action = make(first)\n{replacement}\naction() }}"
        );
        let unit = accepted(&source);
        let body = named(&unit, "run").typed().body();
        let maker = code(&unit, "make");
        let calls = body.expressions().iter().filter_map(|(id, expression)| {
            matches!(&expression.kind, ExpressionKind::Call { target: Origin::Declaration(selected), .. } if *selected == maker).then_some(id)
        }).collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        let receiver = body
            .expressions()
            .iter()
            .find_map(|(id, expression)| {
                matches!(
                    &expression.kind,
                    ExpressionKind::Call {
                        kind: crate::value_access::CallKind::Value,
                        ..
                    }
                )
                .then_some(id)
            })
            .unwrap();
        let site = ValueApplication {
            owner: code(&unit, "run").into(),
            call: receiver,
        };
        let recipe = unit.invocation_recipe(site).unwrap();
        let expected = calls[(2 - alternatives)..]
            .iter()
            .map(|&call| FunctionWitness {
                origin: Origin::Application(ValueApplication {
                    owner: site.owner,
                    call,
                }),
                invocation: nocter_model::ValueInvocation::Read,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(recipe.callees, expected);
    }
}

#[test]
fn receiver_producer_terms_keep_invocation_views_separate_from_source_retirement() {
    use nocter_model::ValueInvocation;
    for (contract, invocation, view) in [
        ("func", "action^()", ValueInvocation::Read),
        ("mut", "action()", ValueInvocation::Edit),
        ("once", "action^()", ValueInvocation::Take),
    ] {
        let source = format!(
            "func first(): i32 {{ 42 }}\nfunc make(): {contract}(): i32 {{ first }}\nfunc run(): i32 {{ var action = make()\n{invocation} }}"
        );
        let unit = accepted(&source);
        let maker = call(&unit, "run", "make");
        let receiver = call(&unit, "run", "first");
        let site = ValueApplication {
            owner: code(&unit, "run").into(),
            call: receiver,
        };
        let recipe = unit.invocation_recipe(site).unwrap();
        assert_eq!(
            recipe.callees,
            BTreeSet::from([FunctionWitness {
                origin: Origin::Application(ValueApplication {
                    owner: site.owner,
                    call: maker
                }),
                invocation: view,
            }])
        );
    }
}

#[test]
fn a_pending_receiver_reservation_cannot_be_overwritten_by_a_later_operand() {
    let source = "func first(value: i32): i32 { value }\nfunc second(value: i32): i32 { value + 1 }\nfunc make(input: func(i32): i32): func(i32): i32 { let n = 0\n(n; value: i32): i32 { n + value } }\nfunc run(): i32 { var action = make(first)\naction^(if true { action = make(second)\n42 } else { 42 }) }";
    let (sources, tree) = input(source);
    assert!(matches!(SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. }))
        if diagnostics.iter().any(|diagnostic| diagnostic.code() == "E0396")));
}

#[test]
fn prepared_receiver_is_not_replaced_by_later_updates_to_another_value_of_the_same_type() {
    let unit = accepted(
        "func first(value: i32): i32 { value }\nfunc second(value: i32): i32 { value + 1 }\nfunc make(input: func(i32): i32): func(i32): i32 { let n = 0\n(n; value: i32): i32 { n + value } }\nfunc run(): i32 { var action = make(first)\nvar other = make(first)\naction^(if true { other = make(second)\n42 } else { 42 }) }",
    );
    let body = named(&unit, "run").typed().body();
    let maker = code(&unit, "make");
    let calls = body.expressions().iter().filter_map(|(id, expression)| {
        matches!(&expression.kind, ExpressionKind::Call { target: Origin::Declaration(selected), .. } if *selected == maker).then_some((id, expression.ty.access().unwrap().data()))
    }).collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|(_, data)| *data == calls[0].1));
    let receiver = body
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(
                &expression.kind,
                ExpressionKind::Call {
                    kind: crate::value_access::CallKind::Value,
                    ..
                }
            )
            .then_some(id)
        })
        .unwrap();
    let site = ValueApplication {
        owner: code(&unit, "run").into(),
        call: receiver,
    };
    let recipe = unit.invocation_recipe(site).unwrap();
    assert_eq!(
        recipe.callees,
        BTreeSet::from([FunctionWitness {
            origin: Origin::Application(ValueApplication {
                owner: site.owner,
                call: calls[0].0
            }),
            invocation: nocter_model::ValueInvocation::Read,
        }])
    );
}
