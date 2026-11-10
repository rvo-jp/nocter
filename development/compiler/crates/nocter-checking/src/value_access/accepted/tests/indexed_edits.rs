//! Complete indexed places lend edit inputs without becoming produced Copy temporaries.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
#[test]
fn selected_edits_change_listed_repeated_and_noncopy_elements() {
    for values in ["[40, 40]", "[40; 2]"] {
        let source = format!(
            "const func increase(edit value: i32): void {{ value += 2 }}\nconst func work(): i32 {{ var values = {values}\nincrease(values[0])\nvalues[0] }}\nconst ANSWER: i32 = work()"
        );
        assert_eq!(answer(&source, "ANSWER"), Datum::integer(42));
    }
    let source = "struct Item { value: i32 }\nconst func replace(edit item: Item): void { item = Item { value: 42 } }\nconst func work(): i32 { var values = [Item { value: 1 }, Item { value: 2 }]\nreplace(values[0])\nvalues[0].value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn edit_parameters_and_whole_array_edit_aliases_retain_their_update_authority() {
    let source = "const func increase(edit value: i32): void { value += 2 }\nconst func change(edit values: [i32; 2]): void { increase(values[0]) }\nconst func work(): i32 { var values = [40, 7]\nedit view = values\nchange(view)\nincrease(view[1])\nview[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn later_index_mutation_does_not_retarget_a_prepared_edit() {
    let source = "const func change(edit index: usize): i32 { index = 1\n0 }\nconst func set(edit value: i32, extra: i32): void { value = 42 + extra }\nconst func work(): (i32, i32, usize) { var values = [0, 7]\nvar index: usize = 0\nset(values[index], change(index))\n(values[0], values[1], index) }\nconst ANSWER: (i32, i32, usize) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![
            Datum::integer(42),
            Datum::integer(7),
            Datum::integer(1)
        ])
    );
}
#[test]
fn edit_reservations_allow_later_reads_but_forbid_invalidating_writes() {
    let source = "const func inspect(values: [i32; 2]): i32 { values[0] }\nconst func set(edit value: i32, previous: i32): void { value = previous + 2 }\nconst func work(): i32 { var values = [40, 7]\nset(values[0], inspect(values))\nvalues[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = "func replace(edit values: [i32; 2]): i32 { values = [0, 0]\n0 }\nfunc set(edit value: i32, other: i32): void {}\nfunc work(): void { var values = [1, 2]\nset(values[0], replace(values)) }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn read_only_owners_aliases_parameters_and_statics_cannot_gain_update_authority() {
    for source in [
        "func change(edit value: i32): void {}\nfunc work(): void { let values = [1, 2]\nchange(values[0]) }",
        "func change(edit value: i32): void {}\nfunc work(values: [i32; 2]): void { change(values[0]) }",
        "func change(edit value: i32): void {}\nfunc work(): void { var values = [1, 2]\nread view = values\nchange(view[0]) }",
        "static VALUES: [i32; 2] = [1, 2]\nfunc change(edit value: i32): void {}\nfunc work(): void { change(VALUES[0]) }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::InvalidUpdate),
            "{source}"
        );
    }
}
#[test]
fn an_unproved_element_overlap_conflicts_with_a_retained_read() {
    let source = "func change(edit value: i32): void {}\nfunc work(i: usize, j: usize): i32 { var values = [1, 2]\nread selected = values[i]\nchange(values[j])\nselected }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn distinct_outer_fields_can_supply_selected_read_and_edit_inputs_together() {
    let source = "struct Row { value: i32 }\nstruct Pair { left: [Row; 1]\nright: [i32; 1] }\nconst func add(edit value: i32, amount: i32): void { value += amount }\nconst func apply(edit pair: Pair): void { add(pair.left[0].value, pair.right[0]) }\nconst func work(): i32 { var pair = Pair { left: [Row { value: 40 }], right: [2] }\napply(pair)\npair.left[0].value }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn interrupted_preparation_does_not_invoke_a_selected_editor() {
    let source = "const func set(edit value: i32, edit calls: i32, other: i32): void { calls += 1\nvalue = 99 }\nconst func work(edit calls: i32): i32 { var values = [0, 1]\nset(values[0], calls, if true { return 42 } else { 0 })\n0 }\nconst func main(): i32 { var calls = 0\nlet result = work(calls)\nresult + calls }\nconst ANSWER: i32 = main()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn editing_a_captured_element_requires_an_editing_closure_invocation() {
    let source = "func increase(edit value: i32): void { value += 1 }\nfunc work(): i32 { let values = [40, 7]\nvar next = (values;): i32 { increase(values[0])\nvalues[0] }\nnext() }";
    let unit = accepted(source);
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        nocter_model::ValueInvocation::Edit
    );
    let invalid = source.replace("var next", "let next");
    let (sources, tree) = input(&invalid);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::InvalidUpdate)
    );
}
