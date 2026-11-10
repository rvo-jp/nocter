//! Scoped indexed updates retain exact addresses independently of conservative conflict regions.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
#[test]
fn scoped_edits_keep_selected_indices_and_independent_array_snapshots() {
    for values in ["[10, 20]", "[10; 2]"] {
        let source = format!(
            "const func work(): (i32, i32, usize) {{ var values = {values}\nlet snapshot = values\nvar index: usize = 0\nedit selected = values[index]\nindex = 1\nselected += 32\n(values[0], snapshot[0], index) }}\nconst ANSWER: (i32, i32, usize) = work()"
        );
        assert_eq!(
            answer(&source, "ANSWER"),
            Datum::aggregate(vec![
                Datum::integer(42),
                Datum::integer(10),
                Datum::integer(1)
            ])
        );
    }
}
#[test]
fn scoped_record_replacement_and_field_updates_target_the_referent() {
    let source = "struct Item { value: i32 }\nconst func work(): i32 { var values = [Item { value: 1 }, Item { value: 2 }]\nedit item = values[0]\nitem = Item { value: 40 }\nedit field = item.value\nfield += 2\nfield }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let partial = "struct Pair { first: i32\nsecond: i32 }\nconst func work(): i32 { var pair = Pair { first: 0, second: 42 }\nedit first = pair.first\nfirst = pair.second^\nfirst }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(partial, "ANSWER"), Datum::integer(42));
}
#[test]
fn scoped_edits_through_edit_parameters_keep_the_original_numeric_index() {
    let source = "const func change(edit values: [i32; 2]): void { var index: usize = 0\nedit selected = values[index]\nindex = 1\nselected = 42 }\nconst func work(): i32 { var values = [0, 7]\nchange(values)\nvalues[0] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn read_aliases_and_computed_values_cannot_become_scoped_edit_authority() {
    for source in [
        "func work(): void { let values = [1, 2]\nedit selected = values[0] }",
        "func work(): void { var values = [1, 2]\nread view = values\nedit selected = view[0] }",
        "func work(): void { var values = [[1, 2], [3, 4]]\nedit row = values[0]\nread view = row\nedit selected = view[0] }",
        "func work(): void { edit selected = [1; 2][0] }",
        "const VALUES: [i32; 2] = [1, 2]\nfunc work(): void { edit selected = VALUES[0] }",
        "static VALUES: [i32; 2] = [1, 2]\nfunc work(): void { edit selected = VALUES[0] }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. })) if error.rule() == crate::value_access::Rule::InvalidUpdate),
            "{source}"
        );
    }
}
#[test]
fn scoped_aliases_protect_their_owner_and_retained_read_children() {
    for source in [
        "func work(): i32 { var values = [1, 2]\nedit selected = values[0]\nread view = selected\nselected = 42\nview }",
        "func work(i: usize, j: usize): i32 { var values = [1, 2]\nread view = values[i]\nedit selected = values[j]\nselected = 42\nview }",
        "func work(): void { var values = [1, 2]\nedit selected = values[0]\ndrop values\nselected = 42 }",
        "func inspect(values: [i32; 2]): i32 { values[0] }\nfunc work(): void { var values = [1, 2]\nedit selected = values[0]\nlet _ = inspect(values)\nselected = 42 }",
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
fn replacing_a_scoped_error_keeps_one_parent_owner_and_an_exact_alias_target() {
    use super::support::steps;
    let unit = accepted(
        "func work(take original: error, take replacement: error): void { var values = [original^]\nedit selected = values[0]\nselected = replacement^ }",
    );
    let query = named(&unit, "work");
    let alias = query
        .typed()
        .body()
        .locals()
        .iter()
        .find_map(|(id, local)| {
            (local.kind == nocter_model::ValueBindingKind::EditAlias).then_some(id)
        })
        .unwrap();
    let trace = steps(query);
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::ReplaceReferent { destination, payload, .. } if destination.holder() == Holder::Local(alias) && destination.fields().is_root() && payload.disposal() == Disposal::OwnedError)).count(), 1);
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::Dispose { payload, .. } if matches!(unit.program().data_schema(payload.data()), Some(nocter_declarations::value_access::PayloadSchema::Array(_))))).count(), 1);
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::DisposeArrayElements { .. }))
    );
}
#[test]
fn scoped_element_updates_select_an_editing_capture_contract() {
    let source = "func work(): i32 { let values = [40, 7]\nvar next = (values;): i32 { edit selected = values[0]\nselected += 1\nselected }\nnext() }";
    let unit = accepted(source);
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        nocter_model::ValueInvocation::Edit
    );
}
#[test]
fn long_mixed_projections_carry_update_demand_without_recursive_scheduling() {
    let mut data = String::from("i32");
    for _ in 0..128 {
        data = format!("[({data}, bool); 1]");
    }
    accepted(&format!(
        "func work(edit values: {data}): void {{ edit selected = values{}\nselected = 42 }}",
        "[0].0".repeat(128)
    ));
}
