//! Required execution consumes selected complete contracts while unrelated headers remain absent.
use super::*;
use crate::value_access::tests::{bound, input};
use nocter_declarations::value_access::PayloadSchema;
use std::fmt::Write as _;

#[test]
fn required_values_prepare_deep_shared_headers_without_recursive_queries_or_call_depth_charges() {
    const DEPTH: usize = 4096;
    let mut source = String::from("struct R0 {}\n");
    for level in 1..=DEPTH {
        writeln!(
            source,
            "struct R{level} {{ left: R{}\nright: R{} }}",
            level - 1,
            level - 1
        )
        .unwrap();
    }
    writeln!(source, "const ANSWER: i32 = if false {{ let _ = deep()\n0 }} else {{ 42 }}\nconst FLAG: bool = true\nconst func deep(): R{DEPTH} {{ let _ = FLAG\nloop {{}} }}\nfunc run(): i32 {{ ANSWER }}").unwrap();
    let (sources, tree) = input(&source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let ids: Vec<_> = headers.callable_ids().collect();
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(256).unwrap(),
        std::num::NonZeroU32::new(2).unwrap(),
    );
    let mut preparation = Preparation::new(
        headers.current(),
        Headers::Publishing(&mut headers),
        limits.into(),
    );
    let program = preparation.context.program();
    let answer = program
        .resolve_code(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    let mut requests = Dependencies::default();
    requests
        .resolve(&mut preparation, Request::Value(answer))
        .unwrap();
    assert_eq!(
        dependencies::value(&requests, answer).unwrap().frozen,
        state::Value::integer(42).freeze()
    );
    assert_eq!(preparation.context.program().records().len(), DEPTH + 1);
    for (id, _) in preparation.context.program().records().iter() {
        assert!(
            matches!(
                requests.completed(&Request::Header(id.into())),
                Some(Fact::Header)
            ),
            "each transitive record must close through the shared query"
        );
    }
    assert_eq!(requests.active_depth(), 0);
    preparation.all(&mut requests, &ids).unwrap();
    let output = preparation.finish(requests).unwrap();
    let headers = headers.finish();
    let context = headers.bind(syntax).unwrap();
    let unit = accepted::Unit::close_evaluation(&context, output).unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&state::Value::integer(42).freeze())
    );
}

#[test]
fn body_value_cycles_unwind_without_publishing_pending_body_or_value_facts() {
    let (sources, tree) =
        input("const func helper(): i32 { loop_value }\nconst loop_value: i32 = helper()");
    let mut headers = declarations::Preparation::prepare(bound(&sources, &tree)).unwrap();
    let mut preparation = Preparation::new(
        headers.current(),
        Headers::Publishing(&mut headers),
        state::Limits::default(),
    );
    let program = preparation.context.program();
    let helper = program
        .resolve_code(program.symbols().get("helper").unwrap())
        .unwrap();
    let initializer = program
        .resolve_code(program.symbols().get("loop_value").unwrap())
        .unwrap();
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(Failure::Evaluation { diagnostic, .. })) =
        preparation.body(&mut requests, helper)
    else {
        panic!("the unfinished body/value cycle must reject")
    };
    assert_eq!(diagnostic.rule(), Rule::DependencyCycle);
    assert!(diagnostic.source().message().contains("body helper"));
    assert!(diagnostic.source().message().contains("value loop_value"));
    assert_eq!(requests.active_depth(), 0);
    assert_eq!(requests.active_keys().len(), 0);
    assert!(dependencies::body(&requests, helper).is_none());
    // Its own typed body finished before closure requested helper. Retain that valid dependency;
    // it grants neither helper-body completion nor initializer execution/whole-unit admission.
    assert!(dependencies::body(&requests, initializer).is_some());
    assert!(dependencies::value(&requests, initializer).is_none());
    assert!(matches!(
        requests.completed(&Request::Header(helper.into())),
        Some(Fact::Header)
    ));
}

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
    let mut requests = Dependencies::default();
    let computed = requests
        .resolve(&mut preparation, Request::Value(answer))
        .unwrap();
    let Fact::Value(computed_value) = &*computed else {
        panic!("a value request must complete with value evidence")
    };
    assert_eq!(computed_value.frozen, state::Value::integer(42).freeze());
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
    let original_body = dependencies::body(&requests, helper)
        .unwrap()
        .program
        .clone();
    preparation.all(&mut requests, &ids).unwrap();
    assert!(Arc::ptr_eq(
        &computed,
        &requests
            .resolve(&mut preparation, Request::Value(answer))
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &original_body,
        &dependencies::body(&requests, helper).unwrap().program
    ));
    assert!(preparation.context.program().records().is_empty());
    let output = preparation.finish(requests).unwrap();
    let complete = headers.finish();
    let context = complete.bind(syntax).unwrap();
    assert!(context.program().headers_complete());
    assert_eq!(context.program().records().len(), 1);
    let unit = accepted::Unit::close_evaluation(&context, output).unwrap();
    assert_eq!(unit.constant_value(answer), Some(&computed_value.frozen));
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
    let mut requests = Dependencies::default();
    preparation
        .require_header(&mut requests, ids[0].into())
        .unwrap();
    assert_eq!(preparation.context.program().callables().len(), 1);
    assert!(preparation.context.program().records().is_empty());
    assert!(matches!(
        accepted::Unit::prepare(&preparation.context),
        Err(Failure::Integrity(_))
    ));
    preparation.body(&mut requests, ids[0]).unwrap();
    let original_program = dependencies::body(&requests, ids[0])
        .unwrap()
        .program
        .clone();
    preparation
        .require_header(&mut requests, ids[1].into())
        .unwrap();
    assert!(
        dependencies::body(&requests, ids[0])
            .unwrap()
            .bind(&preparation.context)
            .is_ok()
    );
    preparation.all(&mut requests, &ids).unwrap();
    assert!(
        Arc::ptr_eq(
            &dependencies::body(&requests, ids[0]).unwrap().program,
            &original_program
        ),
        "a successful body must not be checked again against the larger header view"
    );
    let evaluation = preparation.finish(requests).unwrap();
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
