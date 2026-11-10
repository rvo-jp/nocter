//! Core unit comparisons retain evaluation and access even though their answers are fixed.
use super::*;
use crate::value_access::evaluation::Rule;
use nocter_constant_evaluation::state::{Rule as ExecutionRule, Value as Constant};
fn answer(source: &str) -> Constant {
    let unit = accepted(source);
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    unit.constant_value(id).unwrap().clone()
}
#[test]
fn unit_comparisons_use_the_selected_fixed_result_in_required_evaluation() {
    assert_eq!(
        answer("const ANSWER: bool = () == ()"),
        Constant::Bool(true)
    );
    assert_eq!(
        answer("const ANSWER: bool = () != ()"),
        Constant::Bool(false)
    );
    let source = "const func first(edit count: i32): void { count = count * 10 + 1 }\nconst func second(edit count: i32): void { count = count * 10 + 2 }\nconst func calculate(): i32 { var count = 0\nif first(count) == second(count) { count } else { 0 } }\nconst ANSWER: i32 = calculate()";
    assert_eq!(answer(source), Constant::I32(12));
    assert_eq!(
        answer(
            "const func calculate(): i32 { let value = 42\nif () != () { drop value }\nvalue }\nconst ANSWER: i32 = calculate()"
        ),
        Constant::I32(42)
    );
}
#[test]
fn unit_comparisons_cannot_erase_reads_of_retired_zero_byte_values() {
    let (sources, tree) =
        input("func run(): bool { let value = ()\nlet retired = value^\nvalue == () }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
}
#[test]
fn unit_comparisons_cannot_erase_operand_failure_or_control_interruption() {
    let (sources, tree) =
        input("const func fail(): void { let _ = 1 / 0 }\nconst ANSWER: bool = fail() == ()");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(
        matches!(Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()), Err(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == Rule::Execution(ExecutionRule::ArithmeticFailure))
    );
    assert_eq!(
        answer(
            "const func calculate(): i32 { let _ = (if true { return 42 } else { () }) == () }\nconst ANSWER: i32 = calculate()"
        ),
        Constant::I32(42)
    );
}

#[test]
fn unit_intrinsics_do_not_grant_equality_to_other_empty_or_mismatched_data() {
    for source in [
        "struct Empty {}\nconst ANSWER: bool = Empty {} == Empty {}",
        "const ANSWER: bool = () == false",
    ] {
        let (sources, tree) = input(source);
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
