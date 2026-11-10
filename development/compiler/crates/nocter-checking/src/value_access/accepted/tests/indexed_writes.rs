//! Complete direct writes keep RHS-first evaluation and one captured target address.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::{Rule as ExecutionRule, Value as Datum};
#[test]
fn direct_assignment_and_compound_updates_preserve_copy_snapshots() {
    for values in ["[10, 20]", "[10; 2]"] {
        let source = format!(
            "const func work(): (i32, i32) {{ var values = {values}\nlet snapshot = values\nvalues[0] = 40\nvalues[0] += 2\n(values[0], snapshot[0]) }}\nconst ANSWER: (i32, i32) = work()"
        );
        assert_eq!(
            answer(&source, "ANSWER"),
            Datum::aggregate(vec![Datum::integer(42), Datum::integer(10)])
        );
    }
}
#[test]
fn replacement_and_rhs_reads_run_before_target_indices() {
    let source = "const func produce(edit log: i32): i32 { log = log * 10 + 1\n42 }\nconst func position(edit log: i32): usize { log = log * 10 + 2\n0 }\nconst func work(): (i32, i32) { var values = [0, 7]\nvar log = 0\nvalues[position(log)] = produce(log)\n(values[0], log) }\nconst ANSWER: (i32, i32) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(12)])
    );
    let source = "const func reset(edit values: [i32; 2]): i32 { values = [40, 7]\n2 }\nconst func work(): i32 { var values = [0, 0]\nvalues[0] += reset(values)\nvalues[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn a_same_array_rhs_is_snapshotted_before_the_write() {
    let source = "const func work(): i32 { var values = [1, 42]\nvalues[0] = values[1]\nvalues[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn noncopy_records_and_mixed_fields_replace_the_exact_selected_referent() {
    let source = "struct Item { values: [i32; 2] }\nconst func change(edit items: [Item; 2]): void { items[0] = Item { values: [1, 40] }\nitems[0].values[1] += 2 }\nconst func work(): i32 { var items = [Item { values: [0, 0] }, Item { values: [0, 0] }]\nchange(items)\nitems[0].values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn interruption_in_the_rhs_skips_the_target_while_target_exit_keeps_rhs_effects() {
    let source = "const func position(edit calls: i32): usize { calls += 1\n0 }\nconst func inner(edit calls: i32): i32 { var values = [0]\nvalues[position(calls)] = if true { return 42 } else { 0 }\n0 }\nconst func work(): i32 { var calls = 0\nlet result = inner(calls)\nresult + calls }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = "const func produce(edit calls: i32): i32 { calls += 1\n0 }\nconst func inner(edit calls: i32): i32 { var values = [0]\nvalues[if true { return 42 } else { 0 }] = produce(calls)\n0 }\nconst func work(): i32 { var calls = 0\nlet result = inner(calls)\nresult + calls }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(43));
}
#[test]
fn required_rhs_failure_precedes_target_bounds_failure() {
    let source = "const func work(): void { var values = [0]\nvalues[1] = 1 / 0 }\nconst ANSWER: void = work()";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(ExecutionRule::ArithmeticFailure))
    );
}
#[test]
fn read_only_computed_or_invalidated_targets_do_not_gain_storage() {
    for source in [
        "func work(): void { let values = [1, 2]\nvalues[0] = 42 }",
        "func work(values: [i32; 2]): void { values[0] += 1 }",
        "func work(): void { var values = [1, 2]\nread view = values\nview[0] = 42 }",
        "func work(): void { [1, 2][0] = 42 }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::NotWritable),
            "{source}"
        );
    }
    let source = "const func consume(take values: [i32; 2]): i32 { 42 }\nfunc work(): void { var values = [1, 2]\nvalues[0] = consume(values^) }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn direct_updates_infer_editing_closure_invocation() {
    let unit = accepted(
        "func work(): i32 { let values = [40, 7]\nvar next = (values;): i32 { values[0] += 1\nvalues[0] }\nnext() }",
    );
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        nocter_model::ValueInvocation::Edit
    );
}
#[test]
fn an_interrupted_target_cleans_the_prepared_noncopy_rhs_without_replacement() {
    use super::support::steps;
    let unit = accepted(
        "func work(take original: error, take replacement: error): void { var values = [original^]\nvalues[if true { return } else { 0 }] = replacement^ }",
    );
    let query = named(&unit, "work");
    let value = query
        .typed()
        .body()
        .statements()
        .iter()
        .find_map(|(_, s)| match s {
            crate::value_access::Statement::Assign { value, .. } => Some(*value),
            _ => None,
        })
        .unwrap();
    let trace = steps(query);
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::Dispose { source, payload } if source.holder() == Holder::Expression(value) && payload.disposal() == Disposal::OwnedError)).count(), 1);
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::ReplaceReferent { .. }))
    );
}

#[test]
fn repeated_loop_targets_select_each_current_index_without_history_keys() {
    let source = "const func work(): i32 { var values = [0; 2]\nvar index: usize = 0\nwhile index < 2 { values[index] = 20\nvalues[index] += 1\nindex += 1 }\nvalues[0] + values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn complete_maximum_arrays_can_replace_one_element_without_expansion() {
    let source = "const func work(): (i32, i32) { var values = [0; 18446744073709551615]\nlet snapshot = values\nvalues[18446744073709551614] = 40\nvalues[18446744073709551614] += 2\n(values[18446744073709551614], snapshot[18446744073709551614]) }\nconst ANSWER: (i32, i32) = work()";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(0)])
    );
    for source in [
        "const func work(): void { var values: [i32; 0] = []\nvalues[0] = 42 }\nconst ANSWER: void = work()",
        "const func work(): void { var values = [0; 18446744073709551615]\nvalues[18446744073709551615] = 42 }\nconst ANSWER: void = work()",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(ExecutionRule::IndexOutOfBounds))
        );
    }
}
#[test]
fn unproved_overlap_blocks_a_write_and_existing_read_children_remain_live() {
    let source = "func work(i: usize, j: usize): i32 { var values = [1, 2]\nread selected = values[i]\nvalues[j] = 42\nselected }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
