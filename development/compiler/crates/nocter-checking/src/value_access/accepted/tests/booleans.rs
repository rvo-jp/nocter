//! Boolean result proofs preserve the actual selected evaluation and its ordinary obligations.
use super::*;
use crate::value_access::evaluation::Rule as EvaluationRule;
use nocter_constant_evaluation::state::{Rule as ExecutionRule, Value as Constant};
fn constant(source: &str) -> Constant {
    let unit = accepted(source);
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    Constant::from_frozen(unit.constant_value(id).unwrap()).unwrap()
}
#[test]
fn boolean_result_proofs_select_access_paths_without_reading_place_contents() {
    for condition in [
        "!true",
        "true && false",
        "false || false",
        "FLAG && false",
        "FLAG || false",
    ] {
        let source = format!(
            "const FLAG: bool = false\nfunc run(): i32 {{ let value = 42\nif {condition} {{ drop value }}\nvalue }}"
        );
        accepted(&source);
    }
    // A variable's contents are unknown to this expression-local proof, even if initialized false.
    let (sources, tree) =
        input("func run(flag: bool): i32 { let value = 42\nif flag { drop value }\nvalue }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
}
#[test]
fn skipped_rhs_breaks_do_not_create_a_normal_loop_exit() {
    for expression in ["false &&", "true ||", "!true &&", "false || true ||"] {
        accepted(&format!(
            "func wait_forever(): i32 {{ loop {{ let _ = {expression} if true {{ break }} else {{ false }} }} }}"
        ));
    }
    // The RHS is reached here; its break still requires a following i32 result.
    let (sources, tree) =
        input("func missing(): i32 { loop { let _ = true && if true { break } else { false } } }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()), Err(Failure::Body { failure: BodyFailure::Rejected(d), .. }) if d.rule() == crate::value_access::Rule::MissingResult)
    );
}
#[test]
fn required_rhs_interruption_has_no_invented_normal_value() {
    accepted("func run(): i32 { let _ = true && if true { return 42 } else { false } }");
    accepted("func run(): i32 { let _ = false || if true { return 42 } else { false } }");
    // With an unknown left value the short-circuit path can still return a bool normally.
    let (sources, tree) = input(
        "func missing(flag: bool): i32 { let _ = flag && if true { return 42 } else { false } }",
    );
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()), Err(Failure::Body { failure: BodyFailure::Rejected(d), .. }) if d.rule() == crate::value_access::Rule::MissingResult)
    );
}
#[test]
fn known_results_keep_left_calls_and_skip_only_the_selected_rhs() {
    let source = "const func touch(edit count: i32): bool { count += 1\nfalse }\nconst func calculate(): i32 { var count = 40\nif touch(count) || true { count += 1 }\nlet _ = !true && touch(count)\ncount }\nconst ANSWER: i32 = calculate()";
    assert_eq!(constant(source), Constant::integer(42));
    assert_eq!(
        constant(
            "const func calculate(flag: bool): i32 { var value = 42\nif flag && (if true { return 42 } else { false }) { value = 0 }\nvalue }\nconst ANSWER: i32 = calculate(false) + calculate(true)"
        ),
        Constant::integer(84)
    );
}
#[test]
fn proof_of_a_boolean_result_does_not_erase_required_arithmetic_failure() {
    let (sources, tree) = input(
        "const func fail(): bool { let _ = 1 / 0\nfalse }\nconst ANSWER: bool = fail() || true",
    );
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()), Err(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == EvaluationRule::Execution(ExecutionRule::ArithmeticFailure))
    );
}
#[test]
fn proved_logical_conditions_reach_capture_invocation_and_loop_completion() {
    accepted("func run(): i32 { while !false { return 42 } }");
    accepted("func run(flag: bool): i32 { while flag || true { return 42 } }");
    accepted(
        "func run(flag: bool): i32 { while flag || (if true { return 42 } else { false }) { return 42 } }",
    );
    let unit = accepted(
        "struct Data { value: i32 }\nfunc run(): i32 { let value = Data { value: 42 }\nlet action = (value^;): i32 { if !true { let moved = value^ }\nvalue.value }\naction() }",
    );
    let closure = *unit.closures().keys().next().unwrap();
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
fn known_false_does_not_hide_a_retired_left_read_or_invalid_rhs() {
    let (sources, tree) = input(
        "func run(): void { let value = false\nlet retired = value^\nlet _ = value && false }",
    );
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
    for expression in ["false && 42", "true || missing"] {
        let source = format!("func run(): void {{ let _ = {expression} }}");
        let (sources, tree) = input(&source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(matches!(
            Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
            Err(Failure::Body {
                failure: BodyFailure::Rejected(_),
                ..
            })
        ));
    }
}
