//! Early callable views, body-prefix replay and complete source admission use one authority.
use super::*;
use crate::value_access::{Query as TypedQuery, Rule, evaluation};
use nocter_constant_evaluation::state;
use std::fmt::Write as _;

#[test]
fn missing_callable_contracts_are_requests_even_in_dead_source_and_cannot_bind_backwards() {
    for body in ["omega()", "if false { let _ = omega() }\n0"] {
        let source = format!("func alpha(): i32 {{ {body} }}\nfunc omega(): i32 {{ 42 }}");
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let ids: Vec<_> = headers.code_ids().collect();
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
        let run = headers.code_ids().next().unwrap();
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
fn reserved_value_and_builtin_names_still_forbid_shadowing() {
    for body in [
        "let Data = 0",
        "let seed = 0\nlet action = (seed; Data: i32): i32 { Data }",
        "let seed = 0\nlet action = (seed; u8: i32): i32 { u8 }",
    ] {
        let source = format!("func Data(): i32 {{ 0 }}\nfunc run(): void {{ {body} }}");
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
    let run = headers.code_ids().next().unwrap();
    let early = headers.context(run).unwrap();
    assert!(early.program().codes().is_complete());
    assert!(early.program().parameters().is_complete());
    assert!(!early.program().headers_complete());
    assert!(TypedQuery::prepare(&early, run).is_ok());
    assert!(matches!(Unit::prepare(&early), Err(Failure::Integrity(_))));
    let complete = headers.finish().unwrap();
    assert!(Unit::prepare(&complete.bind(syntax).unwrap()).is_ok());
}

#[test]
fn whole_unit_closure_rejects_missing_bodies_and_incorrect_initializer_value_coverage() {
    let (sources, tree) = input("const ANSWER: i32 = 42\nfunc run(): i32 { ANSWER }");
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let program = context.program();
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    let run = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    for mutation in 0..4 {
        let mut output = evaluation::prepare(&context, state::Limits::default()).unwrap();
        match mutation {
            0 => {
                output.queries.remove(&run.into());
            }
            1 => {
                output.values.remove(&answer.into());
            }
            2 => {
                output
                    .values
                    .insert(run.into(), output.values[&answer.into()].clone());
            }
            _ => {
                let value = output.values.remove(&answer.into()).unwrap();
                output.values.insert(run.into(), value);
            }
        }
        assert!(
            matches!(
                Unit::close_evaluation(&context, output),
                Err(Failure::Integrity(_))
            ),
            "coverage mutation {mutation}"
        );
    }
}

#[test]
fn progressive_const_bodies_retain_recursive_evaluation_and_sibling_body_invalidation() {
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

#[test]
fn progressive_header_requests_do_not_hide_constant_or_static_cycles() {
    for source in [
        "const A: i32 = helper()\nconst func helper(): i32 { B }\nconst B: i32 = A",
        "const A: i32 = if false { B } else { 42 }\nconst B: i32 = A",
        "static a: i32 = helper()\nconst func helper(): i32 { a }",
    ] {
        let (sources, tree) = input(source);
        let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("the active value cycle must reject: {source}")
        };
        assert_eq!(diagnostic.rule(), evaluation::Rule::DependencyCycle);
        assert!(!diagnostic.source().primary().span().range().is_empty());
    }
}

#[test]
fn early_value_success_does_not_skip_unused_definitions_or_execution_limits() {
    let prefix = "const ANSWER: i32 = helper()\nconst func helper(): i32 { 42 }\n";
    for suffix in [
        "func z_unused(): i32 { true }",
        "const func z_unused(): i32 { runtime() }\nfunc runtime(): i32 { 0 }",
    ] {
        let (sources, tree) = input(&format!("{prefix}{suffix}"));
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(matches!(
            failure,
            SourceFailure::Checking(
                Failure::Body {
                    failure: BodyFailure::Rejected(_),
                    ..
                } | Failure::Const { .. }
            )
        ));
    }
    // Keep condition preparation comfortably bounded and exhaust actual loop execution.
    let (sources, tree) = input(&prefix.replace("{ 42 }", "{ loop {} }"));
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(128).unwrap(),
        std::num::NonZeroU32::new(16).unwrap(),
    );
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        SourceUnit::prepare_with_limits(bound(&sources, &tree), limits)
    else {
        panic!("required execution must retain the operation budget")
    };
    assert_eq!(
        diagnostic.rule(),
        evaluation::Rule::Execution(state::Rule::StepLimit)
    );
}

#[test]
fn ordinary_body_value_requests_restart_outer_work_but_share_nested_work() {
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(64).unwrap(),
        std::num::NonZeroU32::new(16).unwrap(),
    );
    let mut source = String::new();
    for i in 0..32 {
        writeln!(
            source,
            "func f{i:02}(): i32 {{ z{i:02} }}\nconst z{i:02}: i32 = {i}"
        )
        .unwrap();
    }
    let (sources, tree) = input(&source);
    let compiled = SourceUnit::prepare_with_limits(bound(&sources, &tree), limits).unwrap();
    for i in 0..32 {
        let program = compiled.unit().program();
        let id = program
            .resolve_constant(program.symbols().get(&format!("z{i:02}")).unwrap())
            .unwrap();
        assert_eq!(
            compiled.unit().constant_value(id),
            Some(&state::Value::integer(i).freeze())
        );
    }
    let total = (0..32)
        .map(|i| format!("z{i:02}"))
        .collect::<Vec<_>>()
        .join(" + ");
    writeln!(
        source,
        "func a_request(): i32 {{ root }}\nconst root: i32 = {total}"
    )
    .unwrap();
    let (sources, tree) = input(&source);
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        SourceUnit::prepare_with_limits(bound(&sources, &tree), limits)
    else {
        panic!("nested dependencies must consume the same work budget")
    };
    // Metadata preparation and execution share the outer-request policy; either phase may
    // reach its deterministic quota first. Neither may replenish work for nested dependencies.
    assert!(matches!(
        diagnostic.rule(),
        evaluation::Rule::Execution(state::Rule::StepLimit | state::Rule::PlanWorkLimit)
    ));
}

#[test]
fn callable_signature_identity_does_not_cycle_through_an_unused_record_payload() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/signature-record-readiness.nct"
    ));
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    let accepted = SourceUnit::prepare(fixture.syntax()).unwrap();
    let program = accepted.unit().program();
    let record = program
        .resolve_record(program.symbols().get("Data").unwrap())
        .unwrap();
    let field = program.records().get(record).unwrap().fields()[0];
    let data = program.fields().get(field).unwrap().data();
    let Some(nocter_declarations::value_access::PayloadSchema::Array(array)) =
        program.data_schema(data)
    else {
        panic!("evaluated record count");
    };
    assert_eq!(array.length(), 42);
    assert!(program.headers_complete());
}

#[test]
fn actual_record_construction_keeps_its_required_count_dependency_cycle() {
    let source = "struct Data { values: [i32; size()] }\nconst func size(): usize { let value = Data { values: [0] }\n1 }\nfunc run(): usize { size() }";
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        SourceUnit::prepare(fixture.syntax())
    else {
        panic!("actual payload needs its count");
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::evaluation::Rule::DependencyCycle
    );
}

#[test]
fn published_nominal_signatures_request_payloads_when_their_bodies_are_checked() {
    for header in [
        "func inspect(value: Data): void {}",
        "func inspect(): Data { loop {} }",
    ] {
        let source = format!("struct Data {{ value: i32 }}\n{header}");
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let syntax = fixture.syntax();
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let code = headers.code_ids().next().unwrap();
        let early = headers.context(code).unwrap();
        let record = early
            .program()
            .resolve_record(early.program().symbols().get("Data").unwrap())
            .unwrap();
        assert!(early.program().header_ready(code));
        assert!(!early.program().header_ready(record));
        assert!(
            matches!(TypedQuery::prepare(&early, code), Err(BodyFailure::RequiredHeader { declaration, .. }) if declaration == record.into())
        );
        let later = headers.context(record).unwrap();
        assert!(later.program().preserves_headers(early.program()));
        assert!(TypedQuery::prepare(&later, code).is_ok());
    }
}
