//! Selected addresses through ordinary source, const execution and normal call preparation.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::{Rule as ExecutionRule, Value as Datum};
#[test]
fn saved_read_access_keeps_its_selected_element_after_index_mutation() {
    let source = "const func work(): i32 { let values = [10, 20]\nvar index: usize = 0\nread selected = values[index]\nindex = 1\nselected + 32 }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn loop_reexecution_selects_new_elements_without_retargeting_the_current_read() {
    let source = "const func work(): i32 { let values = [10, 32]\nvar index: usize = 0\nvar total = 0\nwhile index < 2 { read selected = values[index]\nindex += 1\ntotal += selected }\ntotal }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn read_arguments_capture_the_address_before_later_arguments_mutate_the_index() {
    let source = "const func change(edit index: usize): i32 { index = 1\n0 }\nconst func inspect(value: i32, extra: i32): i32 { value + extra }\nconst func work(): (i32, usize) { let values = [42, 7]\nvar index: usize = 0\nlet value = inspect(values[index], change(index))\n(value, index) }\nconst ANSWER: (i32, usize) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(1)])
    );
}
#[test]
fn taking_copy_inputs_snapshot_before_later_arguments_replace_the_array() {
    let source = "const func replace(edit values: [i32; 2]): i32 { values = [9, 9]\n0 }\nconst func inspect(take value: i32, extra: i32): i32 { value + extra }\nconst func work(): i32 { var values = [42, 7]\ninspect(values[0], replace(values)) }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = source.replace("inspect(take value", "inspect(value");
    let (sources, tree) = input(&invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn computed_bases_and_nested_indices_supply_values_once() {
    let source = "const func make(edit calls: i32): [i32; 2] { calls += 1\n[calls, 40] }\nconst func work(): i32 { var calls = 0\nlet value = make(calls)[0]\nvalue + calls + [[1, 2], [3, 40]][1][1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn required_bounds_fail_at_the_index_operation_instead_of_publishing_a_value() {
    for source in [
        "const ANSWER: i32 = [1, 2][2]",
        "const func work(): i32 { let values: [i32; 0] = []\nvalues[0] }\nconst ANSWER: i32 = work()",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(ExecutionRule::IndexOutOfBounds)),
            "{source}"
        );
    }
}

#[test]
fn noncopy_elements_can_lend_read_access_and_field_views_keep_the_array_footprint() {
    let source = "struct Item { value: i32 }\nconst func work(): i32 { let values = [Item { value: 10 }, Item { value: 32 }]\nread first = values[0]\nread second = values[1]\nfirst.value + second.value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = "struct Item { value: i32 }\nfunc work(): void { var values = [Item { value: 1 }]\nread selected = values[0]\nread field = selected.value\nvalues = [Item { value: 2 }]\nlet _ = field }";
    let (sources, tree) = input(invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn a_covering_element_read_does_not_protect_unrelated_record_fields() {
    let source = "struct Pair { left: [i32; 1]\nright: [i32; 1] }\nconst func work(): i32 { var pair = Pair { left: [10], right: [0] }\nread selected = pair.left[0]\npair.right = [32]\nselected + pair.right[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn scopes_and_unclosed_writes_cannot_turn_an_existing_element_into_an_editable_copy() {
    for (source, rule) in [
        (
            "func work(index: usize): void { read selected = [1, 2][index] }",
            crate::value_access::Rule::TypeMismatch,
        ),
        (
            "func change(edit value: i32): void {}\nfunc work(take values: [i32; 2]): void { change(values[0]) }",
            crate::value_access::Rule::UnsupportedData,
        ),
        (
            "func work(take values: [i32; 2]): void { let selected = values[0]^ }",
            crate::value_access::Rule::UnsupportedData,
        ),
        (
            "func work(take values: [i32; 2]): void { values[0] = 1 }",
            crate::value_access::Rule::UnsupportedData,
        ),
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == rule),
            "{source}"
        );
    }
    for source in [
        "func work(take values: [i32; 2]): void { read selected = values[0]\ndrop values\nlet _ = selected }",
        "func work(take values: [i32; 2]): void { let saved = values^\nlet _ = values[0] }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{source}"
        );
    }
}
#[test]
fn a_computed_copy_component_can_lend_its_own_editable_temporary() {
    let source = "const func change(edit value: i32): i32 { value += 1\nvalue }\nconst ANSWER: i32 = change([40, 7][0]) + 1";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn immutable_statics_keep_place_identity_while_constants_construct_values() {
    let source = "static VALUES: [i32; 2] = [10, 20]\nconst func work(): i32 { var index: usize = 0\nread selected = VALUES[index]\nindex = 1\nselected + 32 }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    assert_eq!(
        answer(
            "const VALUES: [i32; 2] = [10, 32]\nconst ANSWER: i32 = VALUES[0] + VALUES[1]",
            "ANSWER"
        ),
        Datum::integer(42)
    );
    let invalid =
        "const VALUES: [i32; 2] = [10, 20]\nfunc work(): void { read selected = VALUES[0] }";
    let (sources, tree) = input(invalid);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::TypeMismatch)
    );
}
#[test]
fn zero_sized_comparison_does_not_skip_the_index_bounds_effect() {
    assert_eq!(
        answer("const ANSWER: bool = [(), ()][1] == ()", "ANSWER"),
        Datum::boolean(true)
    );
    let (sources, tree) = input("const ANSWER: bool = [(), ()][2] == ()");
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(ExecutionRule::IndexOutOfBounds))
    );
}
#[test]
fn long_valid_index_chains_prepare_control_without_recursive_postfix_scheduling() {
    let mut data = String::from("i32");
    for _ in 0..256 {
        data = format!("[{data}; 1]");
    }
    let source = format!(
        "func work(values: {data}): i32 {{ values{} }}",
        "[0]".repeat(256)
    );
    accepted(&source);
}

#[test]
fn index_side_effects_cannot_invalidate_the_captured_base() {
    for source in [
        "func replace(edit values: [i32; 2]): usize { values = [9, 9]\n0 }\nfunc work(): i32 { var values = [42, 7]\nvalues[replace(values)] }",
        "func consume(take values: [i32; 2]): usize { 0 }\nfunc work(): i32 { let values = [42, 7]\nvalues[consume(values^)] }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{source}"
        );
    }
}
#[test]
fn interrupted_index_preparation_publishes_no_element_or_call_input() {
    let source = "const func consume(take values: [i32; 2], value: i32): i32 { 0 }\nconst func work(): i32 { let values = [1, 2]\nconsume(values^, [3, 4][if true { return 42 } else { 0 }]) }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
