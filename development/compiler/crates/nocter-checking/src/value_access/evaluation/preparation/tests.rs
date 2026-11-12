//! Required execution consumes selected complete contracts while unrelated headers remain absent.
use super::*;
use crate::value_access::tests::{bound, input};
use nocter_declarations::value_access::PayloadSchema;
use std::{fmt::Write as _, sync::Arc};
mod array_construction;
mod callable_dependencies;
mod captured_execution;
mod local_expressions;

fn assert_answer_42(source: &str) {
    let (sources, tree) = input(source);
    let unit = accepted::SourceUnit::prepare(bound(&sources, &tree))
        .unwrap_or_else(|failure| panic!("{source}: {failure:?}"))
        .into_unit();
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(id),
        Some(&state::Value::integer(42).freeze())
    );
}

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
    let ids: Vec<_> = headers.code_ids().collect();
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
        .resolve_steps(&mut preparation, Request::Value(answer.into()))
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
    let headers = headers.finish().unwrap();
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
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        diagnostic,
        ..
    }))) = preparation.body(&mut requests, helper)
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
    let ids: Vec<_> = headers.code_ids().collect();
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
        .resolve_steps(&mut preparation, Request::Value(answer.into()))
        .unwrap();
    let Fact::Value(computed_value) = &*computed else {
        panic!("a value request must complete with value evidence")
    };
    assert_eq!(computed_value.frozen, state::Value::integer(42).freeze());
    assert_eq!(preparation.context.program().codes().len(), 2);
    assert!(preparation.context.program().codes().get(unused).is_none());
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
            .resolve_steps(&mut preparation, Request::Value(answer.into()))
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &original_body,
        &dependencies::body(&requests, helper).unwrap().program
    ));
    assert!(preparation.context.program().records().is_empty());
    let output = preparation.finish(requests).unwrap();
    let complete = headers.finish().unwrap();
    let context = complete.bind(syntax).unwrap();
    assert!(context.program().headers_complete());
    assert_eq!(context.program().records().len(), 1);
    let unit = accepted::Unit::close_evaluation(&context, output).unwrap();
    assert_eq!(unit.constant_value(answer), Some(&computed_value.frozen));
    assert_eq!(unit.bodies().count(), ids.len());
}

#[test]
fn successful_early_bodies_are_reused_and_their_closure_types_replay_after_late_headers() {
    let source = "struct Late { pair: (bool, bool) }\nfunc alpha(): i32 { let pair = (40, true)\nlet action = (pair;): i32 { if pair.1 { pair.0 + 2 } else { 0 } }\naction() }\nfunc beta(action: func(): i32): i32 { action() }";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let ids: Vec<_> = headers.code_ids().collect();
    let mut preparation = Preparation::new(
        headers.current(),
        Headers::Publishing(&mut headers),
        state::Limits::default(),
    );
    let mut requests = Dependencies::default();
    preparation
        .require_header(&mut requests, ids[0].into())
        .unwrap();
    assert_eq!(preparation.context.program().codes().len(), 1);
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
    let headers = headers.finish().unwrap();
    let context = headers.bind(syntax).unwrap();
    let unit = accepted::Unit::close_evaluation(&context, evaluation).unwrap();
    assert!(unit.bind(&context).is_ok());
    assert_eq!(unit.bodies().count(), 2);
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
fn deferred_invalid_records_do_not_block_independent_values_or_escape_complete_unit_validation() {
    use nocter_declarations::value_access::{DeclarationRule, HeaderPosition};
    for (source, name, primary, recursive) in [
        (
            "struct Left { next: Right }\nstruct Right { back: Left }\nconst ANSWER: i32 = 42",
            "Left",
            "Left",
            true,
        ),
        (
            "struct Bad { next: (i32, Bad) }\nconst ANSWER: i32 = 42",
            "Bad",
            "(i32, Bad)",
            true,
        ),
        (
            "copy struct Bad { payload: (i32, error) }\nconst ANSWER: i32 = 42",
            "Bad",
            "(i32, error)",
            false,
        ),
    ] {
        let (sources, tree) = input(source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let mut preparation = Preparation::new(
            headers.current(),
            Headers::Publishing(&mut headers),
            state::Limits::default(),
        );
        let program = preparation.context.program();
        let answer = program
            .resolve_code(program.symbols().get("ANSWER").unwrap())
            .unwrap();
        let invalid = program
            .resolve_record(program.symbols().get(name).unwrap())
            .unwrap();
        let mut requests = Dependencies::default();
        requests
            .resolve_steps(&mut preparation, Request::Value(answer.into()))
            .unwrap();
        let completed = dependencies::value(&requests, answer).unwrap();
        assert_eq!(completed.frozen, state::Value::integer(42).freeze());
        assert!(preparation.context.program().records().is_empty());
        let failure = preparation
            .require_header(&mut requests, invalid.into())
            .unwrap_err();
        let SourceFailure::Declarations(declarations::Failure::Headers(diagnostics)) = failure
        else {
            panic!("the declaration's own failure must survive header scheduling")
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.violation().position, HeaderPosition::Field(0));
        assert_eq!(
            matches!(
                diagnostic.violation().rule,
                DeclarationRule::RecursivePayload { .. }
            ),
            recursive
        );
        assert_eq!(
            syntax.text_at(diagnostic.source().primary().span().range()),
            Some(primary)
        );
        assert_eq!(requests.active_depth(), 0);
        assert!(
            requests
                .completed(&Request::Header(invalid.into()))
                .is_none()
        );
        assert!(preparation.context.program().records().is_empty());
        assert!(preparation.context.program().fields().is_empty());
        assert!(
            dependencies::value(&requests, answer).is_some(),
            "a completed independent value remains valid"
        );
        drop(preparation);
        assert!(matches!(
            headers.finish(),
            Err(declarations::Failure::Headers(_))
        ));
        assert!(matches!(
            crate::value_access::staged::SourceUnit::prepare(syntax),
            Err(SourceFailure::Declarations(declarations::Failure::Headers(
                _
            )))
        ));
    }
}

#[test]
fn record_rejections_keep_field_ownership_through_body_and_value_requests() {
    for (declaration, expected) in [
        ("copy struct Bad { payload: error }", "error"),
        ("struct Bad { payload: Bad }", "Bad"),
        ("struct Bad { payload: Missing }", "Missing"),
        ("struct Bad { payload: f32 }", "f32"),
    ] {
        let source = format!(
            "{declaration}\nconst ANSWER: i32 = if false {{ let _ = deep()\n0 }} else {{ 42 }}\nconst func deep(): Bad {{ loop {{}} }}"
        );
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            crate::value_access::staged::SourceUnit::prepare(syntax)
        else {
            panic!(
                "a record failure reached through a const body must retain declaration ownership"
            )
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(matches!(
            diagnostic.violation().declaration,
            DeclarationId::Record(_)
        ));
        assert_eq!(
            syntax.text_at(diagnostic.source().primary().span().range()),
            Some(expected)
        );
    }
}

#[test]
fn raw_type_names_are_resolved_only_when_their_declaration_is_requested() {
    use nocter_declarations::value_access::HeaderPosition;
    for (source, name, position, expected) in [
        (
            "struct Invalid { value: Missing }\nconst ANSWER: i32 = 42",
            "Invalid",
            HeaderPosition::Field(0),
            "Missing",
        ),
        (
            "struct Invalid { value: f32 }\nconst ANSWER: i32 = 42",
            "Invalid",
            HeaderPosition::Field(0),
            "f32",
        ),
        (
            "func invalid(action: func(Missing): i32): void {}\nconst ANSWER: i32 = 42",
            "invalid",
            HeaderPosition::Parameter(0),
            "func(Missing): i32",
        ),
        (
            "func invalid(): Missing { loop {} }\nconst ANSWER: i32 = 42",
            "invalid",
            HeaderPosition::Result,
            "Missing",
        ),
    ] {
        let (sources, tree) = input(source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let mut preparation = Preparation::new(
            headers.current(),
            Headers::Publishing(&mut headers),
            state::Limits::default(),
        );
        let program = preparation.context.program();
        let invalid = program
            .declaration(program.symbols().get(name).unwrap())
            .unwrap();
        let answer = program
            .resolve_code(program.symbols().get("ANSWER").unwrap())
            .unwrap();
        let mut requests = Dependencies::default();
        requests
            .resolve_steps(&mut preparation, Request::Value(answer.into()))
            .unwrap();
        assert_eq!(
            dependencies::value(&requests, answer).unwrap().frozen,
            state::Value::integer(42).freeze()
        );
        assert!(!preparation.context.program().header_ready(invalid));
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            preparation.require_header(&mut requests, invalid)
        else {
            panic!("the selected raw type must reject at its own annotation")
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.violation().declaration, invalid);
        assert_eq!(diagnostic.violation().position, position);
        assert_eq!(
            syntax.text_at(diagnostic.source().primary().span().range()),
            Some(expected)
        );
        assert_eq!(requests.active_depth(), 0);
        assert!(requests.completed(&Request::Header(invalid)).is_none());
        drop(preparation);
        assert!(matches!(
            headers.finish(),
            Err(declarations::Failure::Headers(_))
        ));
        assert!(matches!(
            crate::value_access::staged::SourceUnit::prepare(syntax),
            Err(SourceFailure::Declarations(declarations::Failure::Headers(
                _
            )))
        ));
    }
}

fn header_data(
    program: &nocter_declarations::value_access::Program,
    owner: DeclarationId,
    position: nocter_declarations::value_access::HeaderPosition,
) -> nocter_model::TypeId {
    use nocter_declarations::value_access::HeaderPosition;
    match position {
        HeaderPosition::Field(index) => {
            let DeclarationId::Record(id) = owner else {
                panic!("record");
            };
            program
                .fields()
                .get(program.records().get(id).unwrap().fields()[index])
                .unwrap()
                .data()
        }
        HeaderPosition::Parameter(index) => {
            let DeclarationId::Code(id) = owner else {
                panic!("code");
            };
            program
                .parameters()
                .get(program.codes().get(id).unwrap().parameters()[index])
                .unwrap()
                .data()
        }
        HeaderPosition::Result => {
            let DeclarationId::Code(id) = owner else {
                panic!("code");
            };
            program.codes().get(id).unwrap().result()
        }
        HeaderPosition::Name => unreachable!(),
    }
}

#[test]
fn anonymous_header_lengths_use_ordinary_usize_checking_and_execution_before_parent_readiness() {
    use nocter_declarations::value_access::{CodeKind, DeclarationId, HeaderPosition};
    for (source, position) in [
        (
            "struct Data { values: [i32; size(20) + size(22)] }\nconst func size(value: usize): usize { value }",
            HeaderPosition::Field(0),
        ),
        (
            "func f(values: [i32; size(20) + size(22)]): void {}\nconst func size(value: usize): usize { value }",
            HeaderPosition::Parameter(0),
        ),
        (
            "func f(): [i32; if true { size(40) + 2 } else { 0 }] {}\nconst func size(value: usize): usize { value }",
            HeaderPosition::Result,
        ),
    ] {
        let (sources, tree) = input(source);
        let syntax = bound(&sources, &tree);
        let mut headers = declarations::Preparation::prepare(syntax).unwrap();
        let early = headers.current();
        let expression = headers
            .code_ids()
            .find(|&id| {
                matches!(
                    early.code_source(id).unwrap().origin(),
                    SyntaxOrigin::Node(_)
                )
            })
            .unwrap();
        let parent_name = if source.starts_with("struct") {
            "Data"
        } else {
            "f"
        };
        let parent: DeclarationId = early
            .program()
            .declaration(early.program().symbols().get(parent_name).unwrap())
            .unwrap();
        let mut preparation = Preparation::new(
            early,
            Headers::Publishing(&mut headers),
            state::Limits::default(),
        );
        let mut requests = Dependencies::default();
        requests
            .resolve_steps(&mut preparation, Request::Value(expression.into()))
            .unwrap();
        assert_eq!(
            dependencies::value(&requests, expression).unwrap().frozen,
            state::Value::integer(42).freeze()
        );
        let program = preparation.context.program();
        let code = program.codes().get(expression).unwrap();
        assert_eq!(code.name(), None);
        assert_eq!(
            code.kind(),
            CodeKind::TypeExpression(nocter_declarations::value_access::TypeExpressionSource {
                declaration: parent,
                position
            })
        );
        assert!(!program.header_ready(parent));
        assert!(!program.headers_complete());
        assert!(matches!(
            requests.completed(&Request::Body(expression.into())),
            Some(Fact::Body(_))
        ));
        assert_eq!(requests.active_depth(), 0);
        // Only the explicit header request publishes its normalized shape after execution.
        preparation.require_header(&mut requests, parent).unwrap();
        let program = preparation.context.program();
        assert!(program.header_ready(parent));
        let data = header_data(program, parent, position);
        let Some(PayloadSchema::Array(array)) = program.data_schema(data) else {
            panic!("normalized array");
        };
        assert_eq!(array.length(), 42);
    }
}

#[test]
fn anonymous_header_lengths_preserve_ordinary_typing_and_dead_authored_const_failures() {
    for (length, helper, expected) in [
        ("true", "", Some(crate::value_access::Rule::TypeMismatch)),
        (
            "if true { return 42 } else { 0 }",
            "",
            Some(crate::value_access::Rule::InvalidReturn),
        ),
        (
            "if false { runtime() } else { 42 }",
            "\nfunc runtime(): usize { 0 }",
            None,
        ),
        (
            "if false { missing } else { 42 }",
            "",
            Some(crate::value_access::Rule::UnknownName),
        ),
    ] {
        let source = format!("struct Data {{ values: [i32; {length}] }}{helper}");
        let (sources, tree) = input(&source);
        let mut headers = declarations::Preparation::prepare(bound(&sources, &tree)).unwrap();
        let early = headers.current();
        let expression = headers
            .code_ids()
            .find(|&id| {
                matches!(
                    early.code_source(id).unwrap().origin(),
                    SyntaxOrigin::Node(_)
                )
            })
            .unwrap();
        let mut preparation = Preparation::new(
            early,
            Headers::Publishing(&mut headers),
            state::Limits::default(),
        );
        let mut requests = Dependencies::default();
        let Err(QueryError::Computation(SourceFailure::Checking(failure))) =
            requests.resolve_steps(&mut preparation, Request::Value(expression.into()))
        else {
            panic!("ordinary source rejection: {source}");
        };
        match (failure, expected) {
            (
                Failure::Body {
                    failure: super::super::BodyFailure::Rejected(diagnostic),
                    ..
                },
                Some(expected),
            ) => assert_eq!(diagnostic.rule(), expected),
            (Failure::Const { diagnostic, .. }, None) => assert!(matches!(
                diagnostic.rule(),
                crate::value_access::constant::Rule::RuntimeCall(_)
            )),
            other => panic!("the exact ordinary rejection: {other:?}"),
        }
        assert!(
            requests
                .completed(&Request::Value(expression.into()))
                .is_none()
        );
        assert_eq!(requests.active_depth(), 0);
    }
}

#[test]
fn failed_anonymous_length_execution_reports_its_real_expression_root() {
    let source = "struct Data { values: [i32; 1 / 0] }";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let mut headers = declarations::Preparation::prepare(syntax).unwrap();
    let early = headers.current();
    let expression = headers
        .code_ids()
        .find(|&id| {
            matches!(
                early.code_source(id).unwrap().origin(),
                SyntaxOrigin::Node(_)
            )
        })
        .unwrap();
    let mut preparation = Preparation::new(
        early,
        Headers::Publishing(&mut headers),
        state::Limits::default(),
    );
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        callable,
        diagnostic,
    }))) = requests.resolve_steps(&mut preparation, Request::Value(expression.into()))
    else {
        panic!("invalid required arithmetic");
    };
    assert_eq!(callable, expression);
    assert_eq!(
        diagnostic.rule(),
        super::super::Rule::Execution(state::Rule::ArithmeticFailure)
    );
    assert_eq!(diagnostic.source().code(), "E0325");
    assert_eq!(
        syntax.text_at(diagnostic.source().primary().span().range()),
        Some("1 / 0")
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("1 / 0"))
    );
    assert!(
        requests
            .completed(&Request::Value(expression.into()))
            .is_none()
    );
    assert_eq!(requests.active_depth(), 0);
}
