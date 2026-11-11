//! Captured const execution uses ordinary source contracts, environments and selected views.
use super::*;

#[test]
fn captured_read_edit_once_and_consuming_bodies_compute_required_answers() {
    assert_answer_42(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-captured-values.nct"
    )));
}
#[test]
fn a_captured_factory_result_keeps_its_code_and_per_instance_environment() {
    assert_answer_42(
        "const func make(value: i32): func(): i32 { (value;): i32 { value + 2 } }\nconst ANSWER: i32 = make(40)()",
    );
}
#[test]
fn called_captured_bodies_load_their_named_const_helpers() {
    assert_answer_42(
        "const func increment(value: i32): i32 { value + 1 }\nconst func calculate(): i32 { let value = 40\nlet action = (value;): i32 { increment(increment(value)) }\naction() }\nconst ANSWER: i32 = calculate()",
    );
}
#[test]
fn dormant_captured_bodies_are_separate_from_formation_and_cleanup() {
    for helper in [
        "func deferred(): i32 { 1 / 0 }",
        "const func deferred(): i32 { ANSWER }",
    ] {
        assert_answer_42(&format!(
            "{helper}\nconst func calculate(): i32 {{ let value = 40\nlet action = (value;): i32 {{ deferred() }}\ndrop action\n42 }}\nconst ANSWER: i32 = calculate()"
        ));
    }
}
#[test]
fn type_only_callable_results_do_not_force_dormant_scalar_body_execution_dependencies() {
    assert_answer_42(
        "const func helper(value: i32): i32 { let _ = ANSWER\nvalue }\nconst func target(value: i32): i32 { value }\nconst func factory(): func(i32): i32 { let items = [1, 2]\nvar index: usize = 0\nwhile index < 2 { let moved = items[index]^\nlet _ = helper(moved)\nindex += 1 }\ntarget }\nconst func calculate(): i32 { let value = 40\nlet action = (value;): i32 { let selected = factory()\nselected(value) }\ndrop action\n42 }\nconst ANSWER: i32 = calculate()",
    );
}

#[test]
fn per_instance_factory_values_do_not_share_captured_state() {
    assert_answer_42(
        "const func make(value: i32): func(): i32 { (value;): i32 { value + 2 } }\nconst func calculate(): i32 { let first = make(40)\nlet next = make(41)\nif first() == 42 && next() == 43 && first() == 42 { 42 } else { 0 } }\nconst ANSWER: i32 = calculate()",
    );
}
#[test]
fn unavailable_captured_calls_keep_transitive_body_causes_in_their_own_code() {
    for body in [
        "let value = 0\nlet action = (value;): i32 { runtime() }\naction()",
        "let value = 0\nlet action = (value;): i32 { if false { runtime() } else { 42 } }\naction()",
        "let value = 0\nlet inner = (value;): i32 { runtime() }\nlet action = (inner;): i32 { inner() }\naction()",
    ] {
        let source = format!(
            "func runtime(): i32 {{ 42 }}\nconst func calculate(): i32 {{ {body} }}\nconst ANSWER: i32 = calculate()"
        );
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            accepted::SourceUnit::prepare(syntax)
        else {
            panic!("actual invocation must reject: {source}");
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
        if body.contains("inner()") {
            assert!(
                diagnostic
                    .source()
                    .notes()
                    .iter()
                    .any(|note| syntax.text_at(note.origin().span().range()) == Some("inner()"))
            );
        }
    }
}
#[test]
fn dormant_captured_ordinary_flow_is_still_required_before_unit_publication() {
    let source = "struct Item { value: i32 }\nconst func calculate(): i32 { let item = Item { value: 42 }\nlet action = (item^;): i32 { let moved = item^\nitem.value }\ndrop action\n42 }\nconst ANSWER: i32 = calculate()";
    let (sources, tree) = input(source);
    assert!(matches!(
        accepted::SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn captured_execution_failures_report_the_real_body_and_budget() {
    for (body, rule) in [
        ("1 / 0", state::Rule::ArithmeticFailure),
        ("let items = [1]\nitems[2]", state::Rule::IndexOutOfBounds),
        ("loop {}", state::Rule::StepLimit),
    ] {
        let source = format!(
            "const func calculate(): i32 {{ let value = 40\nlet action = (value;): i32 {{ {body} }}\naction() }}\nconst ANSWER: i32 = calculate()"
        );
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
            std::num::NonZeroU64::new(200).unwrap(),
            std::num::NonZeroU32::new(8).unwrap(),
        );
        let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
            accepted::SourceUnit::prepare_with_limits(syntax, limits)
        else {
            panic!("actual body failure: {source}");
        };
        assert_eq!(diagnostic.rule(), Rule::Execution(rule));
        if rule == state::Rule::ArithmeticFailure {
            assert_eq!(
                syntax.text_at(diagnostic.source().primary().span().range()),
                Some("1 / 0")
            );
        }
        assert!(
            diagnostic
                .source()
                .notes()
                .iter()
                .any(|note| syntax.text_at(note.origin().span().range()) == Some("action()"))
        );
    }
}

#[test]
fn captured_recursion_uses_source_call_depth_without_adapter_frames() {
    let source = "const func recur(value: i32): i32 { let action = (value;): i32 { recur(value) }\naction() }\nconst ANSWER: i32 = recur(0)";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(1000).unwrap(),
        std::num::NonZeroU32::new(3).unwrap(),
    );
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        accepted::SourceUnit::prepare_with_limits(syntax, limits)
    else {
        panic!("actual call-depth failure");
    };
    assert_eq!(
        diagnostic.rule(),
        Rule::Execution(state::Rule::CallDepthLimit)
    );
    assert_eq!(
        syntax.text_at(diagnostic.source().primary().span().range()),
        Some("recur(value)")
    );
}

#[test]
fn captured_counts_keep_exact_body_dependencies_and_expression_scope() {
    let source = "const func make(value: usize): func(): usize { (value;): usize { value + 2 } }\nstruct Data { values: [i32; make(40)()] }\nfunc unrelated(): i32 { 0 }";
    let (sources, tree) = input(source);
    let unit = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    for (changed, reusable) in [
        (source.replace("value + 2", "value + 3"), false),
        (source.replace("{ 0 }", "{ 1 }"), true),
    ] {
        let (sources, tree) = input(&changed);
        assert_eq!(
            unit.declarations().bind(bound(&sources, &tree)).is_ok(),
            reusable
        );
    }
    assert_answer_42(
        "const ANSWER: i32 = if true { let values: [i32; if true { let value: usize = 40\nlet action = (value;): usize { value + 2 }\naction() } else { 0 }] = [0; 42]\n42 } else { 0 }",
    );
}

#[test]
fn empty_and_multiple_owned_capture_answers_match_their_declared_constant() {
    assert_answer_42(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/owned-environments.nct"
    )));
}
