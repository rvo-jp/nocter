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
        "const ANSWER: void = [(); 18446744073709551615][18446744073709551615]",
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
fn scoped_inputs_do_not_grant_update_authority_or_survive_owner_retirement() {
    for (source, rule) in [
        (
            "func work(index: usize): void { read selected = [1, 2][index] }",
            crate::value_access::Rule::TypeMismatch,
        ),
        (
            "func change(edit value: i32): void {}\nfunc work(values: [i32; 2]): void { change(values[0]) }",
            crate::value_access::Rule::InvalidUpdate,
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
    assert_eq!(
        answer(
            "const ANSWER: bool = [(); 18446744073709551615][18446744073709551614] == ()",
            "ANSWER"
        ),
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

#[test]
fn mixed_member_and_index_reads_keep_saved_addresses_without_copying_intermediates() {
    let source = "struct Row { values: [i32; 2] }\nconst func work(): i32 { let rows = [Row { values: [1, 10] }, Row { values: [2, 32] }]\nvar index: usize = 0\nread first = ((rows[index]).values)[1]\nindex = 1\nfirst + rows[index].values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn selected_field_reads_reserve_while_taking_copy_fields_snapshot() {
    let source = "struct Row { value: i32 }\nconst func replace(edit rows: [Row; 1]): i32 { rows = [Row { value: 9 }]\n0 }\nconst func inspect(take value: i32, extra: i32): i32 { value + extra }\nconst func work(): i32 { var rows = [Row { value: 42 }]\ninspect(rows[0].value, replace(rows)) }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = source.replace("inspect(take value", "inspect(value");
    let (sources, tree) = input(&invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn selected_fields_keep_covering_footprints_and_independent_outer_fields() {
    let source = "struct Row { value: i32 }\nstruct Pair { rows: [Row; 1]\nother: i32 }\nconst func work(): i32 { var pair = Pair { rows: [Row { value: 10 }], other: 0 }\nread selected = pair.rows[0].value\npair.other = 32\nselected + pair.other }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = "struct Row { value: i32 }\nfunc work(): void { var rows = [Row { value: 1 }]\nread selected = rows[0].value\nrows = [Row { value: 2 }]\nlet _ = selected }";
    let (sources, tree) = input(invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn computed_selected_fields_remain_values_and_noncopy_reads_do_not_gain_ownership() {
    let source = "copy struct Row { value: i32 }\nconst func change(edit value: i32): i32 { value += 1\nvalue }\nconst ANSWER: i32 = change([Row { value: 40 }][0].value) + 1";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    for (source, rule) in [
        (
            "copy struct Row { value: i32 }\nfunc work(): void { read selected = ([Row { value: 42 }][0]).value }",
            crate::value_access::Rule::TypeMismatch,
        ),
        (
            "struct Resource { value: i32 }\nstruct Row { resource: Resource }\nfunc work(rows: [Row; 1]): Resource { rows[0].resource }",
            crate::value_access::Rule::MissingTransfer,
        ),
        (
            "struct Row { value: i32 }\nfunc change(edit value: i32): void {}\nfunc work(rows: [Row; 1]): void { change(rows[0].value) }",
            crate::value_access::Rule::InvalidUpdate,
        ),
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == rule),
            "{source}"
        );
    }
}
#[test]
fn long_alternating_member_index_chains_schedule_each_projection_iteratively() {
    let mut data = String::from("i32");
    for _ in 0..128 {
        data = format!("[({data}, bool); 1]");
    }
    accepted(&format!(
        "func work(values: {data}): i32 {{ values{} }}",
        "[0].0".repeat(128)
    ));
}

#[test]
fn mixed_projection_indices_run_once_in_order_and_interruption_stops_the_chain() {
    let source = "struct Row { values: [i32; 1] }\nconst func choose(edit calls: i32, digit: i32): usize { calls = calls * 10 + digit\n0 }\nconst func work(): i32 { let rows = [Row { values: [30] }]\nvar calls = 0\nlet selected = rows[choose(calls, 1)].values[choose(calls, 2)]\nselected + calls }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = "struct Row { values: [i32; 1] }\nconst func work(): i32 { let rows = [Row { values: [30] }]\nrows[0].values[if true { return 42 } else { 0 }] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn a_retained_indexed_callee_prevents_overlapping_replacement_during_inputs() {
    let source = "func work(): i32 { let amount = 2\nlet action = (amount; value: i32): i32 { amount + value }\nvar callbacks = [action]\ncallbacks[0](if true { callbacks = [action]\n40 } else { 0 }) }";
    let (sources, tree) = input(source);
    let result = SourceUnit::prepare(bound(&sources, &tree));
    assert!(
        matches!(&result, Err(SourceFailure::Checking(Failure::Flow { .. }))),
        "{result:?}"
    );
    let source = "struct Item { value: i32 }\nfunc work(): i32 { let item = Item { value: 42 }\nlet action = (item^;): i32 { let owned = item^\nowned.value }\nlet callbacks = [action^]\nlet first = callbacks[0]^()\ncallbacks[0]^() }";
    let (sources, tree) = input(source);
    let result = SourceUnit::prepare(bound(&sources, &tree));
    assert!(
        matches!(&result, Err(SourceFailure::Checking(Failure::Flow { .. }))),
        "{result:?}"
    );
}

#[test]
fn adapted_callable_element_transfers_preserve_prefix_handoff_and_cancellation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/adapted-element-transfers.nct"
    ));
    let unit = accepted(source);
    let cancelled = super::support::steps(named(&unit, "cancelled"));
    assert!(!cancelled.iter().any(|step| matches!(
        step,
        Step::MoveArrayElement { .. } | Step::AdaptFunction { .. } | Step::Invoke { .. }
    )));
}

#[test]
fn specializing_a_callback_effect_does_not_hide_a_real_prefix_counter_write() {
    let source = "func reset(edit index: usize): void { index = 0 }\nfunc invoke(take callback: once(edit usize): void, edit index: usize): void { callback^(index) }\nfunc work(): void { let callbacks = [reset; 3]\nvar index: usize = 0\nwhile index < 3 { invoke(callbacks[index]^, index)\nindex += 1 } }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn view_adaptation_does_not_make_repeated_element_transfers_disjoint() {
    let source = "func pair(take left: once(): i32, take right: once(): i32): i32 { left^() + right^() }\nfunc work(): void { let value = 1\nlet action = (value;): i32 { value }\nlet callbacks = [action; 2]\nvar index: usize = 0\nwhile index < 2 { let unused = pair(callbacks[index]^, callbacks[index]^)\nindex += 1 } }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
