//! Required call closure follows resolved invocation, not dormant function-value formation.
use super::*;

#[test]
fn a_factory_only_result_call_loads_its_resolved_named_body() {
    assert_answer_42(
        "const func increment(value: i32): i32 { value + 1 }\nconst func factory(): func(i32): i32 { increment }\nconst ANSWER: i32 = factory()(41)",
    );
}
#[test]
fn a_dormant_const_value_does_not_create_an_initializer_body_cycle() {
    assert_answer_42(
        "const func deferred(): i32 { ANSWER }\nconst func calculate(): i32 { let unused = deferred\ndrop unused\n42 }\nconst ANSWER: i32 = calculate()",
    );
}

#[test]
fn resolved_named_targets_close_transitive_and_recursive_call_bodies() {
    for source in [
        "const func increment(value: i32): i32 { value + 1 }\nconst func twice(value: i32): i32 { increment(increment(value)) }\nconst func factory(): func(i32): i32 { twice }\nconst ANSWER: i32 = factory()(40)",
        "const func descend(value: i32): i32 { if value == 0 { 42 } else { descend(value - 1) } }\nconst func factory(): func(i32): i32 { descend }\nconst ANSWER: i32 = factory()(4)",
    ] {
        assert_answer_42(source);
    }
}
#[test]
fn a_factory_only_result_call_can_supply_an_independent_local_count() {
    assert_answer_42(
        "const func increment(value: usize): usize { value + 1 }\nconst func factory(): func(usize): usize { increment }\nconst ANSWER: i32 = if true { let values: [i32; factory()(41)] = [0; 42]\n42 } else { 0 }",
    );
}

#[test]
fn resolved_target_effects_close_before_numeric_prefix_admission() {
    assert_answer_42(
        "const func increment(value: i32): i32 { value + 1 }\nconst func factory(): func(i32): i32 { increment }\nconst func calculate(): i32 { let items = [1, 2]\nvar index: usize = 0\nvar total = 0\nwhile index < 2 { let selected = items[index]^\ntotal += factory()(selected)\nindex += 1 }\nif total == 5 { 42 } else { 0 } }\nconst ANSWER: i32 = calculate()",
    );
}

#[test]
fn a_late_body_failure_keeps_its_resolved_call_cause_and_unwinds_requests() {
    let source = "const NEED: i32 = 1 / 0\nconst func deferred(): i32 { NEED }\nconst func factory(): func(): i32 { deferred }\nconst ANSWER: i32 = factory()()";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let id = context
        .program()
        .resolve_constant(context.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    let mut preparation = Preparation::new(context, Headers::Closed, state::Limits::default());
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        diagnostic,
        ..
    }))) = requests.resolve_steps(&mut preparation, Request::Value(id.into()))
    else {
        panic!("actual required arithmetic failure");
    };
    assert_eq!(
        diagnostic.rule(),
        Rule::Execution(state::Rule::ArithmeticFailure)
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("factory()()"))
    );
    assert_eq!(requests.active_depth(), 0);
    assert_eq!(requests.active_keys().len(), 0);
    assert!(dependencies::value(&requests, id).is_none());
}

#[test]
fn resolved_count_supplier_bodies_guard_header_reuse_without_unrelated_bodies() {
    let source = "const func increment(value: usize): usize { value + 1 }\nconst func factory(): func(usize): usize { increment }\nstruct Data { values: [i32; factory()(41)] }\nfunc unrelated(): i32 { 0 }";
    let (sources, tree) = input(source);
    let unit = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    for (changed, reusable) in [
        (source.replace("value + 1", "value + 2"), false),
        (source.replace("{ 0 }", "{ 1 }"), true),
    ] {
        let (sources, tree) = input(&changed);
        assert_eq!(
            unit.declarations().bind(bound(&sources, &tree)).is_ok(),
            reusable,
            "{changed}"
        );
    }
}

#[test]
fn conditional_const_callable_inputs_execute_through_exact_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-callable-inputs.nct"
    ));
    let mut failures = Vec::new();
    for function in [
        "reading",
        "editing",
        "taking",
        "anonymous",
        "interrupted",
        "calculate",
    ] {
        let source = source.replace(
            "const ANSWER: i32 = calculate()",
            &format!("const ANSWER: i32 = {function}()"),
        );
        let (sources, tree) = input(&source);
        match accepted::SourceUnit::prepare(bound(&sources, &tree)) {
            Ok(unit) => {
                let unit = unit.into_unit();
                let id = unit
                    .program()
                    .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
                    .unwrap();
                assert_eq!(
                    unit.constant_value(id),
                    Some(&state::Value::integer(42).freeze()),
                    "{function}"
                );
            }
            Err(failure) => failures.push(format!("{function}: {failure:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_bound_runtime_only_callback_keeps_the_generic_operation_and_caller_cause() {
    let source = "func runtime(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }\nconst ANSWER: i32 = apply(runtime)";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        accepted::SourceUnit::prepare(syntax)
    else {
        panic!("missing authored capability");
    };
    assert!(matches!(
        diagnostic.rule(),
        crate::value_access::constant::Rule::RuntimeCall(_)
    ));
    assert_eq!(
        syntax.text_at(diagnostic.source().primary().span().range()),
        Some("action(42)")
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("apply(runtime)"))
    );
}

#[test]
fn an_unavailable_bound_closure_retains_its_local_fixed_operation_cause() {
    let source = "func runtime(): i32 { 42 }\nconst func apply(action: func(): i32): i32 { action() }\nconst func calculate(): i32 { let value = 0\nlet action = (value;): i32 { if false { runtime() } else { 42 } }\napply(action) }\nconst ANSWER: i32 = calculate()";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        accepted::SourceUnit::prepare(syntax)
    else {
        panic!("fixed runtime source operation");
    };
    assert!(matches!(
        diagnostic.rule(),
        crate::value_access::constant::Rule::CapturedCall(_)
    ));
    assert_eq!(
        syntax.text_at(diagnostic.source().primary().span().range()),
        Some("action()")
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("runtime()"))
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("apply(action)"))
    );
}

#[test]
fn an_unused_runtime_callback_body_does_not_create_a_required_value_cycle() {
    assert_answer_42(
        "func deferred(value: i32): i32 { let _ = ANSWER\nvalue }\nconst func ignore(action: func(i32): i32): i32 { 42 }\nconst ANSWER: i32 = ignore(deferred)",
    );
}

#[test]
fn bound_callback_failures_keep_actual_sites_and_recover_the_same_request_coordinator() {
    for (body, expected, failure_site) in [
        ("value / 0", state::Rule::ArithmeticFailure, "value / 0"),
        // Repeated entry into this empty block consumes the operation budget.
        ("loop {}", state::Rule::StepLimit, "{}"),
    ] {
        let source = format!(
            "const func target(value: i32): i32 {{ {body} }}\nconst func apply(action: func(i32): i32): i32 {{ action(42) }}\nconst ANSWER: i32 = apply(target)\nconst RECOVERY: i32 = 42"
        );
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let headers = declarations::Query::prepare(syntax).unwrap();
        let context = headers.bind(syntax).unwrap();
        let program = context.program();
        let answer = program
            .resolve_constant(program.symbols().get("ANSWER").unwrap())
            .unwrap();
        let recovery = program
            .resolve_constant(program.symbols().get("RECOVERY").unwrap())
            .unwrap();
        let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
            std::num::NonZeroU64::new(128).unwrap(),
            std::num::NonZeroU32::new(8).unwrap(),
        );
        let mut preparation = Preparation::new(context, Headers::Closed, limits.into());
        let mut requests = Dependencies::default();
        let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
            diagnostic,
            ..
        }))) = requests.resolve_steps(&mut preparation, Request::Value(answer.into()))
        else {
            panic!("actual callback failure: {body}");
        };
        assert_eq!(diagnostic.rule(), Rule::Execution(expected));
        assert_eq!(
            syntax.text_at(diagnostic.source().primary().span().range()),
            Some(failure_site)
        );
        for call in ["action(42)", "apply(target)"] {
            assert!(
                diagnostic
                    .source()
                    .notes()
                    .iter()
                    .any(|note| syntax.text_at(note.origin().span().range()) == Some(call)),
                "{call}"
            );
        }
        assert_eq!(requests.active_depth(), 0);
        assert_eq!(requests.active_keys().len(), 0);
        assert!(dependencies::value(&requests, answer).is_none());
        assert_eq!(preparation.budget.storage_used(), 0);
        preparation.budget.restart_work();
        requests
            .resolve_steps(&mut preparation, Request::Value(recovery.into()))
            .unwrap();
        assert_eq!(
            dependencies::value(&requests, recovery).unwrap().frozen,
            state::Value::integer(42).freeze()
        );
        // A completed cached value retains its own metered payload until the cache is dropped.
        assert!(preparation.budget.storage_used() > 0);
        drop(requests);
        assert_eq!(preparation.budget.storage_used(), 0);
    }
}

#[test]
fn condition_expansion_exhaustion_is_distinct_and_the_same_request_can_recover() {
    let source = UNUSED_CALLBACK_PERMUTATIONS;
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let program = context.program();
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(256).unwrap(),
        std::num::NonZeroU32::new(8).unwrap(),
    );
    let mut preparation = Preparation::new(context, Headers::Closed, limits.into());
    preparation.condition_work = state::WorkBudget::new(std::num::NonZeroU64::new(8).unwrap());
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        diagnostic,
        ..
    }))) = requests.resolve_steps(&mut preparation, Request::Value(answer.into()))
    else {
        panic!("bounded analysis of all witness permutations");
    };
    assert_eq!(diagnostic.rule(), Rule::ConditionWorkLimit);
    assert_eq!(diagnostic.source().code(), "E0322");
    assert!(
        syntax
            .text_at(diagnostic.source().primary().span().range())
            .unwrap()
            .starts_with("permute(")
    );
    assert_eq!(requests.active_depth(), 0);
    assert_eq!(requests.active_keys().len(), 0);
    assert!(dependencies::value(&requests, answer).is_none());
    assert_eq!(preparation.budget.storage_used(), 0);
    // Retain the execution quota: spending more condition work must not spend source operations.
    preparation.condition_work = state::WorkBudget::new(std::num::NonZeroU64::new(1000).unwrap());
    requests
        .resolve_steps(&mut preparation, Request::Value(answer.into()))
        .unwrap();
    assert_eq!(
        dependencies::value(&requests, answer).unwrap().frozen,
        state::Value::integer(42).freeze()
    );
    drop(requests);
    assert_eq!(preparation.budget.storage_used(), 0);
}

const UNUSED_CALLBACK_PERMUTATIONS: &str = "func first(): void {}\nfunc second(): void {}\nfunc third(): void {}\nfunc fourth(): void {}\nconst func permute(a: func(): void, b: func(): void, c: func(): void, d: func(): void, count: i32): i32 { if count == 0 { 42 } else { let _ = permute(b, a, c, d, count - 1)\nlet _ = permute(a, c, b, d, count - 1)\npermute(a, b, d, c, count - 1) } }\nconst ANSWER: i32 = permute(first, second, third, fourth, 0)";

#[test]
fn unused_callback_permutations_share_body_discovery_without_sharing_const_or_plan_proofs() {
    let (sources, tree) = input(UNUSED_CALLBACK_PERMUTATIONS);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let program = context.program();
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    let permute = program
        .resolve_code(program.symbols().get("permute").unwrap())
        .unwrap();
    let mut preparation = Preparation::new(context, Headers::Closed, state::Limits::default());
    let mut requests = Dependencies::default();
    preparation.body(&mut requests, answer).unwrap();
    preparation.body(&mut requests, permute).unwrap();
    preparation.condition_work = state::WorkBudget::new(std::num::NonZeroU64::new(8).unwrap());
    let GraphProgress::Ready(ready) = preparation.graph(&requests, answer.into()).unwrap() else {
        panic!("existing ordinary drafts close body discovery");
    };
    assert_eq!(ready.invoked.len(), 2);
    assert!(ready.invoked.contains(&answer.into()));
    assert!(ready.invoked.contains(&permute.into()));
    assert!(dependencies::value(&requests, answer).is_none());
    // Input representation/cleanup still gets its full concrete-instance check afterwards.
    let checked =
        accepted::admit_resolved(&preparation.context, ready.resolved, Some(&ready.invoked))
            .unwrap();
    let Err(Failure::Evaluation { diagnostic, .. }) = accepted::check_required_const(
        &preparation.context,
        &checked,
        answer.into(),
        &mut preparation.condition_work,
    ) else {
        panic!("dependency projection is not a shared eligibility proof");
    };
    assert_eq!(diagnostic.rule(), Rule::ConditionWorkLimit);
}

#[test]
fn interrupted_parameter_invocation_needs_environment_operations_but_no_invocation_capability() {
    for (parameter, callable, receiver, move_argument) in [
        ("", "func", "action", ""),
        ("take ", "once", "action^", "^"),
    ] {
        let prefix = format!(
            "func runtime(value: i32): i32 {{ value + 1 }}\nconst func stop({parameter}action: {callable}(i32): i32): i32 {{ {receiver}(if true {{ return 42 }} else {{ return 0 }}) }}\n"
        );
        assert_answer_42(&format!("{prefix}const ANSWER: i32 = stop(runtime)"));
        assert_answer_42(&format!(
            "{prefix}const func calculate(): i32 {{ let payload = 40\nlet action = (payload; value: i32): i32 {{ runtime(value) + payload }}\nstop(action{move_argument}) }}\nconst ANSWER: i32 = calculate()"
        ));
    }
}
