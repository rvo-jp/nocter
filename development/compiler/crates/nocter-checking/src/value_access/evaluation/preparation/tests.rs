//! Required execution consumes selected complete contracts while unrelated headers remain absent.
use super::*;
use crate::value_access::tests::{bound, input};
use nocter_declarations::value_access::PayloadSchema;

#[test]
fn required_values_execute_before_unrelated_headers_and_retain_their_successful_bodies() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/early-value-late-headers.nct"
    ));
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let ids: Vec<_> = headers.callable_ids().collect();
    let mut preparation = Preparation::new(
        headers.current(),
        Headers::Publishing(&mut headers),
        state::Limits::default(),
    );
    let code = |name| {
        preparation
            .context
            .program()
            .resolve_code(preparation.context.program().symbols().get(name).unwrap())
            .unwrap()
    };
    let answer = code("ANSWER");
    let helper = code("helper");
    let unused = code("z_unused");
    let mut values = Values::default();
    let computed = values.resolve(&mut preparation, answer).unwrap();
    assert_eq!(computed.frozen, state::Value::integer(42).freeze());
    assert_eq!(preparation.context.program().callables().len(), 2);
    assert!(
        preparation
            .context
            .program()
            .callables()
            .get(unused)
            .is_none()
    );
    assert!(preparation.context.program().records().is_empty());
    assert!(!preparation.context.program().headers_complete());
    let original_body = preparation.queries[&helper].program.clone();
    preparation.all(&mut values, &ids).unwrap();
    assert!(Arc::ptr_eq(
        &computed,
        &values.resolve(&mut preparation, answer).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &original_body,
        &preparation.queries[&helper].program
    ));
    assert!(preparation.context.program().records().is_empty());
    let output = preparation.finish(values);
    let complete = headers.finish();
    let context = complete.bind(syntax).unwrap();
    assert!(context.program().headers_complete());
    assert_eq!(context.program().records().len(), 1);
    let unit = accepted::Unit::close_evaluation(&context, output).unwrap();
    assert_eq!(unit.constant_value(answer), Some(&computed.frozen));
    assert_eq!(unit.bodies().len(), ids.len());
}

#[test]
fn successful_early_bodies_are_reused_and_their_closure_types_replay_after_late_headers() {
    let source = "struct Late { pair: (bool, bool) }\nfunc alpha(): i32 { let pair = (40, true)\nlet action = (pair;): i32 { if pair.1 { pair.0 + 2 } else { 0 } }\naction() }\nfunc beta(action: func(): i32): i32 { action() }";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let ids: Vec<_> = headers.callable_ids().collect();
    let mut preparation = Preparation::new(
        headers.current(),
        Headers::Publishing(&mut headers),
        state::Limits::default(),
    );
    preparation.require_header(ids[0].into()).unwrap();
    assert_eq!(preparation.context.program().callables().len(), 1);
    assert!(preparation.context.program().records().is_empty());
    assert!(matches!(
        accepted::Unit::prepare(&preparation.context),
        Err(Failure::Integrity(_))
    ));
    let mut values = Values::default();
    preparation.body(&mut values, ids[0]).unwrap();
    let original_program = preparation.queries[&ids[0]].program.clone();
    preparation.require_header(ids[1].into()).unwrap();
    assert!(
        preparation.queries[&ids[0]]
            .bind(&preparation.context)
            .is_ok()
    );
    preparation.all(&mut values, &ids).unwrap();
    assert!(
        Arc::ptr_eq(&preparation.queries[&ids[0]].program, &original_program),
        "a successful body must not be checked again against the larger header view"
    );
    let evaluation = preparation.finish(values);
    let headers = headers.finish();
    let context = headers.bind(syntax).unwrap();
    let unit = accepted::Unit::close_evaluation(&context, evaluation).unwrap();
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
