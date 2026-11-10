//! Early callable views, body-prefix replay and complete source admission use one authority.
use super::*;
use crate::value_access::{Query as TypedQuery, Rule, evaluation};
use nocter_constant_evaluation::state;
use nocter_declarations::value_access::PayloadSchema;
use std::{collections::BTreeMap, sync::Arc};

#[test]
fn successful_early_bodies_are_reused_and_their_closure_types_replay_after_late_headers() {
    let source = "struct Late { pair: (bool, bool) }\nfunc alpha(): i32 { let pair = (40, true)\nlet action = (pair;): i32 { if pair.1 { pair.0 + 2 } else { 0 } }\naction() }\nfunc beta(action: func(): i32): i32 { action() }";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let ids: Vec<_> = headers.callable_ids().collect();
    let early = headers.context(ids[0]).unwrap();
    assert_eq!(early.program().callables().len(), 1);
    assert!(early.program().records().is_empty());
    assert!(matches!(Unit::prepare(&early), Err(Failure::Integrity(_))));
    let typed = TypedQuery::prepare(&early, ids[0]).unwrap();
    let original_program = typed.program.clone();
    let later = headers.context(ids[1]).unwrap();
    assert!(typed.bind(&later).is_ok());
    let headers = headers.finish();
    let context = headers.bind(syntax).unwrap();
    let evaluation = evaluation::prepare(
        &context,
        state::Limits::default(),
        BTreeMap::from([(ids[0], typed)]),
    )
    .unwrap();
    assert!(
        Arc::ptr_eq(&evaluation.queries[&ids[0]].program, &original_program),
        "a successful body must not be checked again against the larger header view"
    );
    let unit = Unit::close_evaluation(&context, evaluation).unwrap();
    assert!(unit.bind(&context).is_ok());
    assert_eq!(unit.bodies().len(), 2);
    let (&code, _) = unit.closures().iter().next().unwrap();
    let environment = unit.program().closure(code).unwrap().environment();
    let Some(PayloadSchema::Aggregate(schema)) = unit.program().data_schema(environment) else {
        panic!("tuple environment");
    };
    assert_eq!(
        schema
            .components()
            .map(|c| unit.program().data_kind(c.data).unwrap())
            .collect::<Vec<_>>(),
        [
            nocter_model::BuiltinType::I32,
            nocter_model::BuiltinType::Bool
        ]
    );
}

#[test]
fn missing_callable_contracts_are_requests_even_in_dead_source_and_cannot_bind_backwards() {
    for body in ["omega()", "if false { let _ = omega() }\n0"] {
        let source = format!("func alpha(): i32 {{ {body} }}\nfunc omega(): i32 {{ 42 }}");
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let ids: Vec<_> = headers.callable_ids().collect();
        let early = headers.context(ids[0]).unwrap();
        assert!(
            matches!(TypedQuery::prepare(&early, ids[0]), Err(BodyFailure::RequiredHeader { declaration, .. }) if declaration == ids[1].into())
        );
        let later = headers.context(ids[1]).unwrap();
        let typed = TypedQuery::prepare(&later, ids[0]).unwrap();
        assert!(matches!(
            typed.bind(&early),
            Err(BodyFailure::ChangedAuthority)
        ));
        let foreign = declarations::Query::prepare(syntax).unwrap();
        assert!(matches!(
            typed.bind(&foreign.bind(syntax).unwrap()),
            Err(BodyFailure::ChangedAuthority)
        ));
    }
}

#[test]
fn readiness_does_not_hide_declaration_names_or_turn_missing_names_into_requirements() {
    for (body, rule) in [
        ("let omega = 1", Rule::ShadowedName),
        ("missing", Rule::UnknownName),
    ] {
        let source = format!("func alpha(): void {{ {body} }}\nfunc omega(): void {{}}");
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == rule)
        );
    }
    for source in [
        "func alpha(): i32 { omega() }\nfunc omega(): i32 { 42 }",
        "func alpha(): i32 { z_value }\nstatic z_value: i32 = 42",
        "func alpha(): i32 { z_value }\nconst z_value: i32 = 42",
    ] {
        let (sources, tree) = input(source);
        let compiled = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
        let context = compiled
            .declarations()
            .bind(bound(&sources, &tree))
            .unwrap();
        assert!(compiled.unit().bind(&context).is_ok());
    }
}

#[test]
fn unready_records_are_requested_by_construction_annotations_and_closure_inputs() {
    for body in [
        "let value = Data { value: 42 }\nvalue.value",
        "let value: Data = Data { value: 42 }\nvalue.value",
        "if false { let value = Data { value: 42 } }\n42",
        "let seed = 0\nlet action = (seed; input: Data): i32 { input.value }\n42",
    ] {
        let source = format!("struct Data {{ value: i32 }}\nfunc run(): i32 {{ {body} }}");
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let run = headers.callable_ids().next().unwrap();
        let early = headers.context(run).unwrap();
        let record = early
            .program()
            .resolve_record(early.program().symbols().get("Data").unwrap())
            .unwrap();
        assert!(early.program().records().is_empty());
        assert!(
            matches!(TypedQuery::prepare(&early, run), Err(BodyFailure::RequiredHeader { declaration, .. }) if declaration == record.into())
        );
        let later = headers.context(record).unwrap();
        let typed = TypedQuery::prepare(&later, run).unwrap();
        assert!(matches!(
            typed.bind(&early),
            Err(BodyFailure::ChangedAuthority)
        ));
        assert!(typed.bind(&later).is_ok());
        assert!(SourceUnit::prepare(syntax).is_ok());
    }
}

#[test]
fn reserved_names_still_forbid_local_and_closure_parameter_shadowing() {
    for body in [
        "let Data = 0",
        "let seed = 0\nlet action = (seed; Data: i32): i32 { Data }",
        "let seed = 0\nlet action = (seed; u8: i32): i32 { u8 }",
    ] {
        let source = format!("struct Data {{}}\nfunc run(): void {{ {body} }}");
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == Rule::ShadowedName)
        );
    }
}

#[test]
fn completing_callables_alone_cannot_admit_a_unit_with_unready_unused_records() {
    let (sources, tree) = input("struct Unused {}\nfunc run(): i32 { 42 }");
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let run = headers.callable_ids().next().unwrap();
    let early = headers.context(run).unwrap();
    assert!(early.program().callables().is_complete());
    assert!(early.program().parameters().is_complete());
    assert!(!early.program().headers_complete());
    assert!(TypedQuery::prepare(&early, run).is_ok());
    assert!(matches!(Unit::prepare(&early), Err(Failure::Integrity(_))));
    let complete = headers.finish();
    assert!(Unit::prepare(&complete.bind(syntax).unwrap()).is_ok());
}

#[test]
fn seed_const_bodies_retain_recursive_evaluation_and_sibling_body_invalidation() {
    let source = "const func alpha(value: i32): i32 { if value == 0 { 42 } else { omega(value - 1) } }\nconst func omega(value: i32): i32 { alpha(value) }\nconst result: i32 = alpha(2)\nfunc run(): i32 { result }";
    let (sources, tree) = input(source);
    let compiled = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    let unit = compiled.unit();
    let result = unit
        .program()
        .resolve_constant(unit.program().symbols().get("result").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(result),
        Some(&state::Value::integer(42).freeze())
    );
    let edited = source.replace("{ alpha(value) }", "{ 0 }");
    let (edited_sources, edited_tree) = input(&edited);
    let context = compiled
        .declarations()
        .bind(bound(&edited_sources, &edited_tree))
        .unwrap();
    let run = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let query = unit.body(run).unwrap().typed();
    assert!(matches!(
        query.bind(&context),
        Err(BodyFailure::ChangedBody)
    ));
}
