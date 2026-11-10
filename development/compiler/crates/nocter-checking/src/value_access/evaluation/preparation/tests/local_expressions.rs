use super::*;
use nocter_diagnostics::SourceDiagnostic;

fn owner(context: &Context<'_>, name: &str) -> AccessCodeId {
    context
        .program()
        .resolve_code(context.program().symbols().get(name).unwrap())
        .unwrap()
}
fn diagnostic(failure: &SourceFailure) -> &SourceDiagnostic {
    match failure {
        SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(error),
            ..
        }) => error.source(),
        SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) => diagnostic.source(),
        SourceFailure::Checking(Failure::Const { diagnostic, .. }) => diagnostic.source(),
        other => panic!("authored source failure: {other:?}"),
    }
}
fn attempt<'s>(
    context: Context<'s>,
    name: &str,
    limits: state::Limits,
) -> (Preparation<'s, 's>, Dependencies, SourceFailure) {
    let id = owner(&context, name);
    let mut preparation = Preparation::new(context, Headers::Closed, limits);
    let mut requests = Dependencies::default();
    let QueryError::Computation(failure) = preparation.body(&mut requests, id).unwrap_err() else {
        panic!("request diagnostics must be projected at the actual source cause");
    };
    assert_eq!(requests.active_depth(), 0);
    assert!(preparation.causes.is_empty());
    (preparation, requests, failure)
}

fn prepared<'s>(context: Context<'s>, name: &str) -> (Preparation<'s, 's>, Dependencies) {
    let id = owner(&context, name);
    let mut preparation = Preparation::new(context, Headers::Closed, state::Limits::default());
    let mut requests = Dependencies::default();
    preparation.body(&mut requests, id).unwrap();
    assert_eq!(requests.active_depth(), 0);
    assert!(preparation.causes.is_empty());
    (preparation, requests)
}

#[test]
fn local_lengths_execute_before_the_enclosing_body_finishes() {
    for result in ["void", "error", "func(): i32"] {
        let text = format!(
            "const BASE: usize = 20\nconst func increment(edit value: usize): usize {{ value += 1\nvalue }}\nfunc run(take unrelated: error): {result} {{ let first: [i32; if true {{ var value: usize = BASE\nlet pair = (value, increment(value))\npair.0 + pair.1 + 1 }} else {{ 0 }}] = if true {{ loop {{}} }} else {{ loop {{}} }}\nlet second: [i32; 6 * 7] = if true {{ loop {{}} }} else {{ loop {{}} }} }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let ids = context
            .body_expressions(owner(&context, "run"))
            .unwrap()
            .ids()
            .collect::<Vec<_>>();
        let run = owner(&context, "run");
        let (preparation, requests) = prepared(context, "run");
        assert_eq!(ids.len(), 2);
        for id in ids {
            let root = ValueCodeRoot::Expression(id);
            assert_eq!(
                dependencies::value(&requests, root).unwrap().frozen,
                state::Value::integer(42).freeze()
            );
            let body = dependencies::body(&requests, root).unwrap();
            assert_eq!(body.code(), root.code());
            assert_eq!(
                body.program().data_kind(body.result_data()),
                Some(nocter_model::BuiltinType::Usize)
            );
            assert!(body.body().inputs().explicit().is_empty());
            assert!(body.body().inputs().environment().is_none());
            body.bind(&preparation.context).unwrap();
        }
        assert!(dependencies::body(&requests, run).is_some());
    }
}

#[test]
fn enclosing_values_and_control_targets_are_not_implicit_expression_inputs() {
    for (prefix, length, expected) in [
        ("let outside: usize = 42", "outside", "E0421"),
        (
            "let outside: usize = 42",
            "if true { let outside: usize = 1\noutside } else { 0 }",
            "E0341",
        ),
        ("", "if true { return 42 } else { 0 }", "E0370"),
        ("", "if true { break } else { 0 }", "E0382"),
        ("", "if true { continue } else { 0 }", "E0382"),
    ] {
        let text = format!(
            "func run(): void {{ {prefix}\nloop {{ let value: [i32; {length}] = if true {{ loop {{}} }} else {{ loop {{}} }} }} }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let (_, _, failure) = attempt(context, "run", state::Limits::default());
        assert_eq!(diagnostic(&failure).code(), expected, "{text}: {failure:?}");
    }
}

#[test]
fn read_edit_and_take_parameters_are_not_available_as_compile_time_arguments() {
    for operation in ["", "edit ", "take "] {
        let text = format!(
            "func run({operation}size: usize): void {{ let value: [i32; size] = if true {{ loop {{}} }} else {{ loop {{}} }} }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let (_, _, failure) = attempt(context, "run", state::Limits::default());
        let error = diagnostic(&failure);
        assert_eq!(error.code(), "E0421", "{failure:?}");
        assert_eq!(
            bound(&sources, &tree).text_at(error.primary().span().range()),
            Some("size")
        );
    }
}

#[test]
fn local_lengths_keep_ordinary_type_const_and_execution_failures() {
    for (length, helper, code) in [
        ("true", "", "E0370"),
        ("-1", "", "E0375"),
        ("1 / 0", "", "E0325"),
        ("missing", "", "E0340"),
        (
            "if false { runtime() } else { 0 }",
            "func runtime(): usize { 0 }",
            "E0421",
        ),
    ] {
        let text = format!(
            "{helper}\nfunc run(): void {{ let value: [i32; {length}] = if true {{ loop {{}} }} else {{ loop {{}} }} }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let (_, _, failure) = attempt(context, "run", state::Limits::default());
        assert_eq!(diagnostic(&failure).code(), code, "{text}: {failure:?}");
        assert!(!diagnostic(&failure).primary().span().range().is_empty());
    }
}

#[test]
fn body_local_type_value_cycles_keep_expression_and_function_roles() {
    let text = "const func run(): usize { let value: [i32; run()] = if true { loop {} } else { loop {} } }";
    let (sources, tree) = input(text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let (_, requests, failure) = attempt(context, "run", state::Limits::default());
    let error = diagnostic(&failure);
    assert_eq!(error.code(), "E0324", "{failure:?}");
    assert!(error.message().contains("body run"));
    assert!(error.message().contains("expression 0 in run"));
    assert_eq!(
        bound(&sources, &tree).text_at(error.primary().span().range()),
        Some("run()")
    );
    assert!(
        requests
            .into_dependencies()
            .iter()
            .all(|node| !matches!(&*node.value, Fact::Value(_)))
    );
}

#[test]
fn completed_local_count_queries_retain_selected_supplier_and_whole_body_guards() {
    let text = "const BASE: usize = 40\nconst func width(): usize { 2 }\nfunc other(): i32 { 7 }\nfunc run(): void { let outside = 10\nlet value: [i32; BASE + width()] = if true { loop {} } else { loop {} } }";
    let (sources, tree) = input(text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let id = context
        .body_expressions(owner(&context, "run"))
        .unwrap()
        .ids()
        .next()
        .unwrap();
    let (preparation, requests) = prepared(context.clone(), "run");
    let output = preparation.finish(requests).unwrap();
    let query = output.queries.get(&ValueCodeRoot::Expression(id)).unwrap();
    for (before, after, valid) in [
        ("BASE: usize = 40", "BASE: usize = 41", false),
        // Call typing can be reused after a callee body edit; executing that call again uses
        // the new body. The value dependency below, rather than the typed call, retains it.
        ("width(): usize { 2 }", "width(): usize { 3 }", true),
        ("outside = 10", "outside = 11", false),
        ("other(): i32 { 7 }", "other(): i32 { 8 }", true),
    ] {
        let (next_sources, next_tree) = input(&text.replace(before, after));
        let next = headers.bind(bound(&next_sources, &next_tree)).unwrap();
        assert_eq!(query.bind(&next).is_ok(), valid, "{before}");
        let root = ValueCodeRoot::Expression(id);
        let (_, fresh) = prepared(next, "run");
        assert_eq!(
            dependencies::value(&fresh, root).unwrap().frozen,
            state::Value::integer(
                if before.starts_with("BASE") || before.starts_with("width") {
                    43
                } else {
                    42
                }
            )
            .freeze(),
            "fresh execution after {before}"
        );
    }
    let (_, requests) = prepared(context.clone(), "run");
    let graph = requests.into_dependencies();
    let root = graph
        .iter()
        .position(|node| node.key == Request::Value(ValueCodeRoot::Expression(id)))
        .unwrap();
    let mut pending = vec![root];
    let mut prerequisites = std::collections::BTreeSet::new();
    while let Some(index) = pending.pop() {
        if prerequisites.insert(index) {
            pending.extend(graph[index].dependencies.iter().copied());
        }
    }
    assert!(
        prerequisites
            .iter()
            .any(|&index| graph[index].key == Request::Body(owner(&context, "width").into()))
    );
    assert!(
        prerequisites
            .iter()
            .any(|&index| graph[index].key == Request::Value(owner(&context, "BASE").into()))
    );
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    assert!(
        output
            .queries
            .contains_key(&ValueCodeRoot::Declaration(owner(&context, "run")))
    );
    assert!(
        !output
            .queries
            .contains_key(&ValueCodeRoot::Declaration(owner(&context, "other")))
    );
    assert!(
        matches!(
            accepted::Unit::close_evaluation(&context, output),
            Err(Failure::Integrity(_))
        ),
        "completed counts and one body cannot publish a Unit with unprepared declarations"
    );
}

#[test]
fn nested_count_requests_keep_distinct_roots_and_enclosing_scope_exclusions() {
    let text = "func run(): void { let outside: usize = 42\nlet value: [i32; if true { let zero: usize = 0\nlet nested: [i32; 6 * 7] = if true { loop {} } else { loop {} }\nzero + 0 } else { 0 }] = if true { loop {} } else { loop {} } }";
    let (sources, tree) = input(text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let ids = context
        .body_expressions(owner(&context, "run"))
        .unwrap()
        .ids()
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(64).unwrap(),
        std::num::NonZeroU32::new(8).unwrap(),
    );
    let (_, requests, failure) = attempt(context, "run", limits.into());
    assert!(
        matches!(&failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })
        if diagnostic.rule() == Rule::Execution(state::Rule::StepLimit)),
        "{failure:?}"
    );
    assert!(dependencies::value(&requests, ValueCodeRoot::Expression(ids[0])).is_none());
    assert_eq!(
        dependencies::value(&requests, ValueCodeRoot::Expression(ids[1]))
            .unwrap()
            .frozen,
        state::Value::integer(42).freeze()
    );
    let (sources, tree) = input(&text.replace("6 * 7", "outside"));
    let failure = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert_eq!(diagnostic(&failure).code(), "E0421");
}

#[test]
fn enclosing_scope_exclusions_survive_a_count_local_closure() {
    let text = "func run(): void { let outside: usize = 42\nlet value: [i32; if true { let seed = 0\nlet callback = (seed; ): usize { outside }\n0 } else { 0 }] = if true { loop {} } else { loop {} } }";
    let (sources, tree) = input(text);
    let failure = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert_eq!(diagnostic(&failure).code(), "E0421", "{failure:?}");
}

#[test]
fn local_count_recursion_and_nontermination_keep_budget_failures_and_source_causes() {
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(64).unwrap(),
        std::num::NonZeroU32::new(4).unwrap(),
    );
    for (length, helper) in [
        ("width()", "const func width(): usize { width() }"),
        ("if true { loop {} } else { 0 }", ""),
    ] {
        let text = format!(
            "{helper}\nfunc run(): void {{ let value: [i32; {length}] = if true {{ loop {{}} }} else {{ loop {{}} }} }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let (_, _, failure) = attempt(context, "run", limits.into());
        let error = diagnostic(&failure);
        assert_eq!(error.code(), "E0322", "{failure:?}");
        let primary = bound(&sources, &tree)
            .text_at(error.primary().span().range())
            .unwrap();
        assert!(!primary.starts_with("func run"), "{primary}");
    }
}

#[test]
fn raised_source_depth_budget_closes_a_deep_constant_chain_without_host_recursion() {
    use std::fmt::Write as _;
    let mut text = String::new();
    for index in 0..300 {
        writeln!(text, "const VALUE{index:03}: i32 = VALUE{:03}", index + 1).unwrap();
    }
    text.push_str("const VALUE300: i32 = 42");
    let (sources, tree) = input(&text);
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(100_000).unwrap(),
        std::num::NonZeroU32::new(512).unwrap(),
    );
    let source = accepted::SourceUnit::prepare_with_limits(bound(&sources, &tree), limits).unwrap();
    let unit = source.unit();
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("VALUE000").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(id),
        Some(&state::Value::integer(42).freeze())
    );
}

#[test]
fn repetition_counts_execute_without_executing_the_runtime_seed() {
    for (count, expected) in [
        ("6 * 7", 42),
        ("width()", 42),
        ("0", 0),
        ("18446744073709551615", u64::MAX),
    ] {
        let text = format!(
            "const func width(): usize {{ 42 }}\nfunc seed(): i32 {{ 7 }}\nfunc run(): void {{ let values = [seed(); {count}] }}"
        );
        let (sources, tree) = input(&text);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let root = ValueCodeRoot::Expression(
            context
                .body_expressions(owner(&context, "run"))
                .unwrap()
                .ids()
                .next()
                .unwrap(),
        );
        let seed = owner(&context, "seed");
        let (_, requests) = prepared(context, "run");
        assert_eq!(
            dependencies::value(&requests, root).unwrap().frozen,
            state::Value::integer(i128::from(expected)).freeze()
        );
        assert!(
            dependencies::body(&requests, seed).is_none(),
            "typing repetition must not execute its runtime seed"
        );
    }
    for (parameters, count, code) in [
        ("size: usize", "size", "E0421"),
        ("", "1 / 0", "E0325"),
        ("", "true", "E0370"),
    ] {
        let text = format!("func run({parameters}): void {{ let values = [7; {count}] }}");
        let (sources, tree) = input(&text);
        let failure = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert_eq!(diagnostic(&failure).code(), code, "{failure:?}");
    }
}

#[test]
fn element_writes_transfers_and_unclosed_compositions_remain_gated() {
    for operation in [
        "edit value = items[0]",
        "let value = items[0]^",
        "items[0] = 1",
        "let value = items[0]()",
    ] {
        let text = format!("func run(items: [i32; 2]): void {{ {operation} }}");
        let (sources, tree) = input(&text);
        let failure = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::UnsupportedData),
            "{operation}: {failure:?}"
        );
    }
}
