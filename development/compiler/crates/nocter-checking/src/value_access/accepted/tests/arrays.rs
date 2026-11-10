//! Source construction and whole-value handoffs, independently of unadmitted element selection.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
fn list(values: &[i32]) -> Datum {
    Datum::aggregate(
        values
            .iter()
            .copied()
            .map(Datum::integer)
            .collect::<Box<[_]>>(),
    )
}
#[test]
fn listed_nested_empty_and_zero_sized_arrays_publish_exact_data() {
    for (source, expected) in [
        ("const ANSWER: [i32; 2] = [20, 22]", list(&[20, 22])),
        (
            "const ANSWER: [[i32; 2]; 2] = [[1, 2], [3, 4]]",
            Datum::aggregate(vec![list(&[1, 2]), list(&[3, 4])]),
        ),
        ("const ANSWER: [i32; 0] = []", list(&[])),
        (
            "const ANSWER: [void; 2] = [(), ()]",
            Datum::aggregate(vec![Datum::unit(); 2]),
        ),
        (
            "const ANSWER: ([i32; 2], bool) = ([20, 22], true)",
            Datum::aggregate(vec![list(&[20, 22]), Datum::boolean(true)]),
        ),
    ] {
        assert_eq!(answer(source, "ANSWER"), expected, "{source}");
    }
}
#[test]
fn whole_array_calls_preserve_copy_snapshots_and_edit_selected_storage() {
    let source = "const func overwrite(edit values: [i32; 2]): void { values = [40, 2] }\nconst func identity(take values: [i32; 2]): [i32; 2] { values^ }\nconst func work(): ([i32; 2], [i32; 2]) { var original = [1, 2]\nlet snapshot = original\noverwrite(original)\n(identity(snapshot), identity(original^)) }\nconst ANSWER: ([i32; 2], [i32; 2]) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![list(&[1, 2]), list(&[40, 2])])
    );
}
#[test]
fn noncopy_components_transfer_once_and_whole_assignment_restores_the_container() {
    let source = "struct Item { number: i32 }\nconst func identity(take values: [Item; 2]): [Item; 2] { values^ }\nconst func work(): [Item; 2] { var values = [Item { number: 1 }, Item { number: 2 }]\nlet saved = identity(values^)\nvalues = [Item { number: 3 }, Item { number: 4 }]\ndrop values\nsaved^ }\nconst ANSWER: [Item; 2] = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![list(&[1]), list(&[2])])
    );
    let invalid = source.replace("identity(values^)", "identity(values)");
    let (sources, tree) = input(&invalid);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::MissingTransfer)
    );
}
#[test]
fn components_execute_in_source_order_before_array_assembly() {
    let source = "const func step(edit log: i32, value: i32): i32 { log = log * 10 + value\nvalue }\nconst func work(): ([i32; 2], i32) { var log = 0\nlet values = [step(log, 1), step(log, 2)]\n(values, log) }\nconst ANSWER: ([i32; 2], i32) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![list(&[1, 2]), Datum::integer(12)])
    );
}
#[test]
fn required_array_lengths_can_execute_bodies_that_construct_and_transfer_arrays() {
    let source = "const func discard(take values: [usize; 2]): usize { 2 }\nconst func width(): usize { let values: [usize; 2] = [1, 2]\ndiscard(values^) }\nconst ANSWER: [i32; width()] = [20, 22]";
    assert_eq!(answer(source, "ANSWER"), list(&[20, 22]));
}
#[test]
fn empty_containment_cycles_remain_valid_source_without_claiming_evaluator_shape_support() {
    let source = "struct Node { next: [Node; 0] }\nfunc empty(): [Node; 0] { [] }";
    accepted(source);
    let (sources, tree) = input(&source.replace("func empty", "const func empty"));
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) if diagnostic.source().code() == "E0421")
    );
}
