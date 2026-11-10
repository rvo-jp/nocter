//! Required source evaluation and final-value publication, including stateful const calls.
use super::*;
use crate::value_access::evaluation::Rule;
use nocter_constant_evaluation::{
    CompileTimeEvaluationLimits,
    state::{Data as ValueData, Rule as ExecutionRule, Value},
};
use std::fmt::Write as _;
use std::num::{NonZeroU32, NonZeroU64};
fn answer(source: &str, name: &str) -> Value {
    let unit = accepted(source);
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get(name).unwrap())
        .unwrap();
    assert!(
        unit.program()
            .resolve(unit.program().symbols().get(name).unwrap())
            .is_none()
    );
    Value::from_frozen(unit.constant_value(id).unwrap()).unwrap()
}
#[test]
fn literals_forward_dependencies_and_runtime_references_publish_values() {
    assert_eq!(
        answer(
            "const ANSWER: i32 = BASE + 2\nconst BASE: i32 = 40\nfunc run(): i32 { ANSWER }",
            "ANSWER"
        ),
        Value::integer(42)
    );
    assert_eq!(
        answer(
            "const ANSWER: bool = true || false\nconst NOTHING: void = ()",
            "ANSWER"
        ),
        Value::boolean(true)
    );
    assert_eq!(answer("const NOTHING: void = ()", "NOTHING"), Value::unit());
}
#[test]
fn editing_calls_preserve_distinct_storage_and_do_not_cache_unit_results() {
    let source = "const func increment(edit value: i32): void { value += 1 }\nconst func calculate(): i32 {\nvar first = 20\nvar second = 20\nincrement(first)\nincrement(second)\nfirst + second\n}\nconst ANSWER: i32 = calculate()";
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}
#[test]
fn stateful_records_moves_aliases_loops_and_copy_snapshots_use_checked_order() {
    let source = "struct Pair {\nleft: i32\nright: i32\n}\nconst func increment(edit value: i32): i32 { value += 1\nvalue }\nconst func select(take pair: Pair): i32 { pair.left^ }\nconst func calculate(): i32 {\nvar pair = Pair { left: 0, right: 2 }\nedit target = pair.left\nwhile target < 20 { let _ = increment(target) }\nlet snapshot = target + increment(target)\nlet selected = select(pair^)\nsnapshot + selected - 20\n}\nconst ANSWER: i32 = calculate()";
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}
#[test]
fn recursive_calls_and_short_circuits_execute_only_selected_paths() {
    let source = "const func total(value: i32): i32 { if value == 0 { 0 } else { value + total(value - 1) } }\nconst func invalid(): bool { let _ = 1 / 0\ntrue }\nconst ANSWER: i32 = total(9)\nconst SAFE: bool = true || invalid()";
    assert_eq!(answer(source, "ANSWER"), Value::integer(45));
}
fn failure(source: &str, limits: impl Into<nocter_constant_evaluation::state::Limits>) -> Rule {
    let (sources, tree) = input(source);
    let query = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let result = Unit::prepare_with_limits(&query.bind(bound(&sources, &tree)).unwrap(), limits);
    let Err(Failure::Evaluation { diagnostic, .. }) = result else {
        panic!("{source}: {result:?}");
    };
    assert!(!diagnostic.source().primary().span().range().is_empty());
    diagnostic.rule()
}
#[test]
fn required_failures_are_not_runtime_fallbacks_or_domain_failures() {
    assert_eq!(
        failure(
            "const BAD: i32 = 1 / 0",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::Execution(ExecutionRule::ArithmeticFailure)
    );
    assert_eq!(
        failure(
            "const A: i32 = B\nconst B: i32 = A",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::DependencyCycle
    );
    assert_eq!(
        failure(
            "const func again(): i32 { again() }\nconst BAD: i32 = again()",
            CompileTimeEvaluationLimits::new(
                NonZeroU64::new(10000).unwrap(),
                NonZeroU32::new(8).unwrap()
            )
        ),
        Rule::Execution(ExecutionRule::CallDepthLimit)
    );
    assert_eq!(
        failure(
            "const func forever(): i32 { loop {} }\nconst BAD: i32 = forever()",
            CompileTimeEvaluationLimits::new(
                NonZeroU64::new(50).unwrap(),
                NonZeroU32::new(8).unwrap()
            )
        ),
        Rule::Execution(ExecutionRule::StepLimit)
    );
    assert_eq!(
        failure(
            "const BAD: error = if true { loop {} } else { loop {} }",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::FinalValue
    );
}

#[test]
fn taking_copy_inputs_and_lending_retired_sources_preserve_supply_choices() {
    assert_eq!(
        answer(
            "const func update(take value: i32): i32 { value += 1\nvalue }\nconst func run(): i32 { let value = 20\nlet next = update(value)\nvalue + next + 1 }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Value::integer(42)
    );
    assert_eq!(
        answer(
            "const func inspect(value: i32): i32 { value }\nconst func run(): i32 { let value = 40\ninspect(value^) + 2 }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn edit_input_replacement_and_alternative_partial_cleanup_follow_the_ledger() {
    assert_eq!(
        answer(
            "struct Pair { left: i32\nright: i32 }\nconst func replace(edit pair: Pair): void { pair = Pair { left: 20, right: 22 } }\nconst func run(): i32 { var pair = Pair { left: 1, right: 2 }\nreplace(pair)\npair.left + pair.right }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Value::integer(42)
    );
    assert_eq!(
        answer(
            "struct Pair { left: i32\nright: i32 }\nconst func run(flag: bool): i32 { let pair = Pair { left: 1, right: 42 }\nif flag { let moved = pair.left^ }\npair.right }\nconst ANSWER: i32 = run(true) + run(false)",
            "ANSWER"
        ),
        Value::integer(84)
    );
}

#[test]
fn empty_records_and_interrupted_argument_preparation_keep_real_owner_presence() {
    assert_eq!(
        answer(
            "struct Empty {}\nconst func inspect(take value: Empty): i32 { 42 }\nconst ANSWER: i32 = inspect(Empty {})",
            "ANSWER"
        ),
        Value::integer(42)
    );
    assert_eq!(
        answer(
            "const func add(left: i32, right: i32): i32 { left + right }\nconst func run(): i32 { add(1, if true { return 42 } else { 2 }) }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn constants_are_fresh_data_and_have_no_source_place_or_function_witness() {
    for body in [
        "let value = ANSWER^",
        "read value = ANSWER",
        "edit value = ANSWER",
        "ANSWER = 0",
        "drop ANSWER",
        "ANSWER()",
    ] {
        let text = format!("const ANSWER: i32 = 42\nfunc run(): void {{ {body} }}");
        let (sources, tree) = input(&text);
        let query = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&query.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { .. })
            ),
            "{text}"
        );
    }
    let unit = accepted("const ANSWER: i32 = 42");
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert!(
        unit.program()
            .function_data(nocter_declarations::value_access::FunctionWitness {
                origin: nocter_model::ValueFunctionOrigin::Declaration(id),
                invocation: nocter_model::ValueInvocation::Read
            })
            .is_none()
    );
}

#[test]
fn initializer_rebinding_reuses_headers_but_recomputes_values_and_rejects_stale_units() {
    let (a, ta) = input("const BASE: i32 = 1\nconst ANSWER: i32 = BASE + 1");
    let headers = declarations::Query::prepare(bound(&a, &ta)).unwrap();
    let old_context = headers.bind(bound(&a, &ta)).unwrap();
    let old = Unit::prepare(&old_context).unwrap();
    let (b, tb) = input("// current source\nconst BASE: i32 = 40\nconst ANSWER: i32 = BASE + 2");
    let current_context = headers.bind(bound(&b, &tb)).unwrap();
    assert!(old.bind(&current_context).is_err());
    let current = Unit::prepare(&current_context).unwrap();
    let id = current
        .program()
        .resolve_constant(current.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(old.constant_value(id), Some(&Value::integer(2).freeze()));
    assert_eq!(
        current.constant_value(id),
        Some(&Value::integer(42).freeze())
    );
}

#[test]
fn execution_diagnostics_keep_the_callee_operation_and_initializer_call_path() {
    let (sources, tree) = input("const func fail(): i32 { 1 / 0 }\nconst BAD: i32 = fail()");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let Err(Failure::Evaluation { diagnostic, .. }) =
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap())
    else {
        panic!("arithmetic failure");
    };
    assert_eq!(
        sources
            .get(tree.source())
            .unwrap()
            .text_at(diagnostic.source().primary().span().range()),
        Some("1 / 0")
    );
    assert!(diagnostic.source().notes().len() >= 2);
}

#[test]
fn dependency_budget_diagnostics_point_to_the_request_that_exhausted_the_budget() {
    let (sources, tree) = input("const A: i32 = B\nconst B: i32 = C\nconst C: i32 = 42");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let limits =
        CompileTimeEvaluationLimits::new(NonZeroU64::new(1).unwrap(), NonZeroU32::new(8).unwrap());
    let Err(Failure::Evaluation { diagnostic, .. }) =
        Unit::prepare_with_limits(&headers.bind(bound(&sources, &tree)).unwrap(), limits)
    else {
        panic!("dependency budget must fail");
    };
    assert_eq!(diagnostic.rule(), Rule::Execution(ExecutionRule::StepLimit));
    assert_eq!(
        sources
            .get(tree.source())
            .unwrap()
            .text_at(diagnostic.source().primary().span().range()),
        Some("C")
    );
}

#[test]
fn folded_runtime_bodies_reject_rebinding_after_a_const_callee_changes() {
    let original = "const func calculate(): i32 { 1 }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { ANSWER }";
    let (a, ta) = input(original);
    let headers = declarations::Query::prepare(bound(&a, &ta)).unwrap();
    let context = headers.bind(bound(&a, &ta)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let (b, tb) = input(&original.replace("{ 1 }", "{ 42 }"));
    let changed = headers.bind(bound(&b, &tb)).unwrap();
    assert!(named(&unit, "run").bind(&changed).is_err());
    assert!(named(&unit, "run").typed().bind(&changed).is_err());
    let current = Unit::prepare(&changed).unwrap();
    let run = named(&current, "run");
    assert!(
        run.typed()
            .body()
            .expressions()
            .iter()
            .any(|(_, expression)| expression.kind
                == crate::value_access::ExpressionKind::Integer(42))
    );
}

#[test]
fn remainder_has_its_own_result_domain_and_division_keeps_its_range_requirement() {
    assert_eq!(
        answer("const ANSWER: i32 = -2147483648 % -1", "ANSWER"),
        Value::integer(0)
    );
    for expression in ["-2147483648 / -1", "42 % 0"] {
        assert_eq!(
            failure(
                &format!("const BAD: i32 = {expression}"),
                CompileTimeEvaluationLimits::default()
            ),
            Rule::Execution(ExecutionRule::ArithmeticFailure)
        );
    }
}

#[test]
fn authored_dependency_cycles_are_rejected_in_unevaluated_branches_and_const_callees() {
    for text in [
        "const A: i32 = if false { A } else { 42 }",
        "const A: i32 = if false { B } else { 42 }\nconst B: i32 = A",
        "const func helper(): i32 { A }\nconst A: i32 = if false { helper() } else { 42 }",
        "const func outer(): i32 { inner() }\nconst func inner(): i32 { if false { A } else { 42 } }\nconst A: i32 = outer()",
    ] {
        assert_eq!(
            failure(text, CompileTimeEvaluationLimits::default()),
            Rule::DependencyCycle,
            "{text}"
        );
    }
    assert_eq!(
        answer(
            "const func first(value: i32): i32 { if value == 0 { 42 } else { second(value - 1) } }\nconst func second(value: i32): i32 { first(value) }\nconst ANSWER: i32 = first(3)",
            "ANSWER"
        ),
        Value::integer(42)
    );
    assert_eq!(
        answer(
            "const ANSWER: i32 = if false { 1 / 0 } else { 42 }",
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn unsupported_final_shapes_fail_locally_even_when_no_value_can_be_produced() {
    assert_eq!(
        failure(
            "const BAD: error = if true { loop {} } else { loop {} }",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::FinalValue
    );
}

#[test]
fn shipped_limits_bound_recursive_execution_and_deep_initializer_dependencies() {
    assert_eq!(
        failure(
            "const func again(): i32 { again() }\nconst BAD: i32 = again()",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::Execution(ExecutionRule::CallDepthLimit)
    );
    let mut text = String::new();
    for index in 0..12 {
        writeln!(text, "const VALUE{index}: i32 = VALUE{}", index + 1).unwrap();
    }
    text.push_str("const VALUE12: i32 = 42");
    assert_eq!(
        failure(
            &text,
            CompileTimeEvaluationLimits::new(
                NonZeroU64::new(10000).unwrap(),
                NonZeroU32::new(8).unwrap()
            )
        ),
        Rule::Execution(ExecutionRule::CallDepthLimit)
    );
    // Pending value dependencies and active callable frames share one stack-depth budget.
    // Each part fits by itself; their combined active depth does not.
    assert_eq!(
        failure(
            "const A: i32 = B\nconst B: i32 = C\nconst C: i32 = count(3)\nconst func count(value: i32): i32 { if value == 0 { 42 } else { count(value - 1) } }",
            CompileTimeEvaluationLimits::new(
                NonZeroU64::new(10000).unwrap(),
                NonZeroU32::new(6).unwrap()
            )
        ),
        Rule::Execution(ExecutionRule::CallDepthLimit)
    );
}

#[test]
fn compiler_initialization_thunks_do_not_introduce_source_return_targets() {
    let (sources, tree) = input("const BAD: i32 = if true { return 1 } else { 2 }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources,&tree)).unwrap()),Err(Failure::Body{failure:BodyFailure::Rejected(diagnostic),..}) if diagnostic.rule()==crate::value_access::Rule::InvalidReturn)
    );
    assert_eq!(
        answer(
            "const func choose(): i32 { if true { return 42 } else { 2 } }\nconst ANSWER: i32 = choose()",
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn cache_guards_include_const_callees_seen_only_in_unevaluated_dependency_paths() {
    let original = "const func helper(): i32 { 42 }\nconst ANSWER: i32 = if false { helper() } else { 42 }\nfunc run(): i32 { ANSWER }";
    let (a, ta) = input(original);
    let headers = declarations::Query::prepare(bound(&a, &ta)).unwrap();
    let context = headers.bind(bound(&a, &ta)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let (b, tb) = input(&original.replace("{ 42 }\nconst", "{ ANSWER }\nconst"));
    let changed = headers.bind(bound(&b, &tb)).unwrap();
    assert!(named(&unit, "run").bind(&changed).is_err());
    assert!(
        matches!(Unit::prepare(&changed),Err(Failure::Evaluation{diagnostic,..}) if diagnostic.rule()==Rule::DependencyCycle)
    );
}

#[test]
fn required_values_select_control_before_initialization_and_access_admission() {
    let unit = accepted(
        "const FLAG: bool = false\nfunc run(): i32 { let value = 42\nif FLAG { drop value }\nvalue }",
    );
    assert!(named(&unit, "run").initialization().violations().is_empty());
    assert_eq!(
        answer(
            "const FLAG: bool = false\nconst func run(): i32 { let value = 42\nif FLAG { drop value }\nvalue }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Value::integer(42)
    );
    let (sources, tree) = input(
        "const FLAG: bool = true\nfunc run(): i32 { let value = 42\nif FLAG { drop value }\nvalue }",
    );
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
}

#[test]
fn required_values_close_normal_completion_and_dead_breaks_during_typing() {
    accepted("const STOP: bool = true\nfunc run(): i32 { while STOP { return 42 } }");
    accepted("const EXIT: bool = false\nfunc wait_forever(): i32 { loop { if EXIT { break } } }");
    let (sources, tree) =
        input("const EXIT: bool = true\nfunc missing(): i32 { loop { if EXIT { break } } }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. }) if diagnostic.rule() == crate::value_access::Rule::MissingResult)
    );
}

#[test]
fn constant_control_facts_reach_capture_invocation_inference() {
    let unit = accepted(
        "const FLAG: bool = false\nstruct Payload { value: i32 }\nfunc run(): i32 { let data = Payload { value: 42 }\nlet action = (data^;): i32 { if FLAG { let moved = data^ }\ndata.value }\naction() }",
    );
    let run = named(&unit, "run");
    let closure = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            if let crate::value_access::ExpressionKind::Closure { closure, .. } = expression.kind {
                Some(closure)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        unit.program()
            .closure(closure)
            .unwrap()
            .contract()
            .invocation,
        nocter_model::ValueInvocation::Read
    );
}

#[test]
fn constant_control_facts_do_not_skip_structural_checks_in_dead_source() {
    let (sources, tree) =
        input("const FLAG: bool = false\nfunc run(): i32 { if FLAG { let value = missing }\n42 }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body {
            failure: BodyFailure::Rejected(_),
            ..
        })
    ));
}

#[test]
fn default_depth_budget_bounds_deep_source_dependency_preparation() {
    let mut source = String::new();
    for index in 0..260 {
        writeln!(source, "const VALUE{index:03}: i32 = VALUE{:03}", index + 1).unwrap();
    }
    source.push_str("const VALUE260: i32 = 42");
    assert_eq!(
        failure(&source, CompileTimeEvaluationLimits::default()),
        Rule::Execution(ExecutionRule::CallDepthLimit)
    );
}

#[test]
fn ordinary_type_and_flow_errors_precede_final_value_domain_rejection() {
    for (source, is_flow) in [
        ("struct Pair { value: i32 }\nconst BAD: Pair = 42", false),
        (
            "struct Pair { value: i32 }\nconst BAD: Pair = if true { let data = Pair { value: 42 }\ndrop data\ndata^ } else { Pair { value: 0 } }",
            true,
        ),
    ] {
        let (sources, tree) = input(source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap());
        if is_flow {
            assert!(matches!(result, Err(Failure::Flow { .. })), "{result:?}");
        } else {
            assert!(matches!(result, Err(Failure::Body { .. })), "{result:?}");
        }
    }
}

#[test]
fn unevaluated_callee_dependency_cycles_keep_the_authored_call_path() {
    let (sources, tree) = input(
        "const func helper(): i32 { ANSWER }\nconst ANSWER: i32 = if false { helper() } else { 42 }",
    );
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let Err(Failure::Evaluation { diagnostic, .. }) =
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap())
    else {
        panic!("cycle");
    };
    assert_eq!(diagnostic.rule(), Rule::DependencyCycle);
    assert!(diagnostic.source().notes().iter().any(|note| {
        sources
            .get(tree.source())
            .unwrap()
            .text_at(note.origin().span().range())
            == Some("helper()")
    }));
}

fn storage_limits(units: u64) -> nocter_constant_evaluation::state::Limits {
    nocter_constant_evaluation::state::Limits::new(
        CompileTimeEvaluationLimits::default(),
        NonZeroU64::new(units).unwrap(),
    )
}
#[test]
fn storage_exhaustion_is_a_local_required_evaluation_diagnostic() {
    for units in [1, 4] {
        let (sources, tree) = input("const BAD: i32 = 42");
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let Err(Failure::Evaluation { diagnostic, .. }) = Unit::prepare_with_limits(
            &headers.bind(bound(&sources, &tree)).unwrap(),
            storage_limits(units),
        ) else {
            panic!("storage exhaustion must reject required evaluation");
        };
        assert_eq!(
            diagnostic.rule(),
            Rule::Execution(ExecutionRule::StorageLimit)
        );
        assert_eq!(diagnostic.source().code(), "E0322");
        assert!(diagnostic.source().message().contains("live-storage"));
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(diagnostic.source().primary().span().range()),
            Some("42")
        );
    }
}
#[test]
fn bounded_loop_storage_is_reused_instead_of_accumulating_allocation_history() {
    let text = "const func count(): i32 { var value = 0\nwhile value < 1000 { value += 1 }\nvalue }\nconst ANSWER: i32 = count()";
    let (sources, tree) = input(text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare_with_limits(
        &headers.bind(bound(&sources, &tree)).unwrap(),
        storage_limits(128),
    )
    .unwrap();
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(id),
        Some(&Value::integer(1000).freeze())
    );
}
#[test]
fn completed_dependencies_and_outer_requests_retain_their_storage_charges() {
    let mut source = String::new();
    for index in 0..128 {
        writeln!(source, "const C{index}: i32 = {index}").unwrap();
    }
    assert_eq!(
        failure(&source, storage_limits(64)),
        Rule::Execution(ExecutionRule::StorageLimit)
    );
    let mut source = String::from("const ANSWER: i32 = C0\n");
    for index in 0..127 {
        writeln!(source, "const C{index}: i32 = C{}", index + 1).unwrap();
    }
    source.push_str("const C127: i32 = 42");
    assert_eq!(
        failure(&source, storage_limits(64)),
        Rule::Execution(ExecutionRule::StorageLimit)
    );
}
#[test]
fn ordinary_source_validation_precedes_evaluator_storage_allocation() {
    let (sources, tree) = input("const BAD: i32 = true");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare_with_limits(
            &headers.bind(bound(&sources, &tree)).unwrap(),
            storage_limits(1)
        ),
        Err(Failure::Body { .. })
    ));
}

#[test]
fn record_constants_are_fresh_noncopy_values_with_nested_unit_fields() {
    let source = "struct Empty {}\nstruct Pair { left: i32\nright: i32\nempty: Empty\nunit: void }\nconst func make(): Pair { var value = Pair { right: 2, left: 40, empty: Empty {}, unit: () }\nvalue.left += 1\nvalue^ }\nconst ORIGINAL: Pair = make()\nconst SECOND: Pair = ORIGINAL\nconst func calculate(): i32 { var first = SECOND\nlet next = ORIGINAL\nfirst.left = 1\nfirst.left + next.left }\nconst ANSWER: i32 = calculate()";
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    let ValueData::Record(fields) = answer(source, "SECOND").data().clone() else {
        panic!("record");
    };
    assert_eq!(fields[0], Value::integer(41));
    assert_eq!(fields[1], Value::integer(2));
    assert_eq!(fields[2], Value::record(Vec::new()));
    assert_eq!(fields[3], Value::unit());
}
#[test]
fn record_constant_names_are_not_places_and_do_not_grant_copy_to_bindings() {
    for body in [
        "read value = DATA",
        "edit value = DATA",
        "let value = DATA^",
        "drop DATA",
        "DATA.left = 1",
        "let first = DATA\nlet second = first",
    ] {
        let source = format!(
            "struct Pair {{ left: i32 }}\nconst DATA: Pair = Pair {{ left: 42 }}\nfunc run(): void {{ {body} }}"
        );
        let (sources, tree) = input(&source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { .. })
            ),
            "{source}"
        );
    }
}
#[test]
fn record_constant_dependencies_keep_cycles_and_source_invalidation() {
    assert_eq!(
        failure(
            "struct Pair { left: i32 }\nconst A: Pair = B\nconst B: Pair = A",
            CompileTimeEvaluationLimits::default()
        ),
        Rule::DependencyCycle
    );
    let original = "struct Pair { left: i32 }\nconst DATA: Pair = Pair { left: 1 }\nfunc run(): i32 { DATA.left }";
    let (sources, tree) = input(original);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()).unwrap();
    let (changed, parsed) = input(&original.replace("left: 1", "left: 42"));
    let context = headers.bind(bound(&changed, &parsed)).unwrap();
    assert!(named(&unit, "run").bind(&context).is_err());
    let current = Unit::prepare(&context).unwrap();
    let id = current
        .program()
        .resolve_constant(current.program().symbols().get("DATA").unwrap())
        .unwrap();
    assert_ne!(unit.constant_value(id), current.constant_value(id));
}

#[test]
fn record_constant_scalar_projections_supply_control_facts_before_flow() {
    accepted(
        "struct Options { flag: bool }\nconst OPTIONS: Options = Options { flag: false }\nfunc run(): i32 { let value = 42\nif OPTIONS.flag { drop value }\nvalue }",
    );
    assert_eq!(
        answer(
            "struct Inner { value: i32 }\nstruct Outer { inner: Inner }\nconst DATA: Outer = Outer { inner: Inner { value: 42 } }\nconst ANSWER: i32 = DATA.inner.value",
            "ANSWER"
        ),
        Value::integer(42)
    );
}
