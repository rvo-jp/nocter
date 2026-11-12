//! Structural disjointness uses evaluated value facts, never source recipe identity.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;

#[test]
fn known_elements_can_supply_independent_edits_and_keep_read_siblings() {
    let source = "const func add(edit left: i32, edit right: i32): void { left += 1\nright += 1 }\nconst func work(): i32 { var values = [20, 20]\nadd(values[0], values[1])\nread first = values[0]\nvalues[1] += 0\nfirst + values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = "const LEFT: usize = 0\nconst RIGHT: usize = 1\nconst func work(): i32 { var values = [20, 22]\nedit left = values[LEFT]\nedit right = values[RIGHT]\nleft += 0\nright += 0\nleft + right }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn unknown_indices_can_be_separated_by_later_record_fields() {
    let source = "struct Row { left: i32\nright: i32 }\nconst func add(edit left: i32, edit right: i32): void { left += 1\nright += 1 }\nconst func change(edit rows: [Row; 2], i: usize, j: usize): void { add(rows[i].left, rows[j].right) }\nconst func work(i: usize, j: usize): i32 { var rows = [Row { left: 20, right: 20 }, Row { left: 20, right: 20 }]\nchange(rows, i, j)\nrows[i].left + rows[j].right }\nconst ANSWER: (i32, i32) = (work(0, 0), work(0, 1))";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(42)])
    );
}
#[test]
fn unknown_indices_can_be_separated_by_later_known_elements() {
    let source = "const func add(edit left: i32, edit right: i32): void { left += 1\nright += 1 }\nconst func work(i: usize, j: usize): i32 { var values = [[20, 20], [20, 20]]\nadd(values[i][0], values[j][1])\nvalues[i][0] + values[j][1] }\nconst ANSWER: (i32, i32) = (work(0, 0), work(0, 1))";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(42)])
    );
}
#[test]
fn a_read_field_can_survive_an_unknown_index_write_to_a_disjoint_field() {
    let source = "struct Row { left: i32\nright: i32 }\nconst func work(i: usize, j: usize): i32 { var rows = [Row { left: 10, right: 0 }, Row { left: 10, right: 0 }]\nread selected = rows[i].left\nrows[j].right = 32\nselected + rows[j].right }\nconst ANSWER: (i32, i32) = (work(0, 0), work(0, 1))";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(42)])
    );
}
#[test]
fn normal_result_singletons_preserve_index_branch_effects() {
    let source = "const func work(flag: bool): i32 { var calls = 0\nvar values = [20, 20]\nedit left = values[if flag { calls += 1\n0 } else { calls += 10\n0 }]\nedit right = values[1]\nleft += 1\nright += 1\nleft + right + calls }\nconst ANSWER: (i32, i32) = (work(true), work(false))";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(43), Datum::integer(52)])
    );
}
#[test]
fn equal_or_unproved_indices_and_whole_parent_invalidation_still_conflict() {
    for source in [
        "func work(): i32 { var values = [20, 22]\nread first = values[0]\nvalues[0] = 42\nfirst }",
        "func work(i: usize, j: usize): i32 { var values = [20, 22]\nread first = values[i]\nvalues[j] = 42\nfirst }",
        "struct Row { left: i32\nright: i32 }\nfunc work(i: usize, j: usize): i32 { var rows = [Row { left: 20, right: 22 }]\nread first = rows[i].left\nrows[j].left = 42\nfirst }",
        "func work(): i32 { var values = [20, 22]\nread first = values[0]\nvalues = [1, 2]\nfirst }",
        "func work(): void { var values = [20, 22]\nedit first = values[0]\ndrop values\nfirst += 1 }",
        "func work(): i32 { var values = [20, 22]\nvalues[if true { values = [1, 2]\n0 } else { 0 }] }",
        "struct Pair { values: [i32; 2] }\nfunc work(): i32 { var pair = Pair { values: [20, 22] }\nlet removed = pair.values^\npair.values[0] }",
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
fn identifying_a_base_does_not_grant_read_access_to_its_active_edit() {
    for source in [
        "func inspect(values: [i32; 2]): void {}\nfunc work(): void { var values = [20, 22]\nedit selected = values[0]\ninspect(values)\nselected += 1 }",
        "func work(): void { var values = [20, 22]\nedit selected = values[0]\nlet snapshot = values\nselected += 1 }",
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
fn later_call_operands_may_update_disjoint_elements_but_not_the_reserved_one() {
    let source = "const func add(edit value: i32, delta: i32): void { value += delta }\nconst func next(edit value: i32): i32 { value += 1\n1 }\nconst func work(): i32 { var values = [20, 20]\nadd(values[0], next(values[1]))\nvalues[0] + values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let invalid = source.replace("next(values[1])", "next(values[0])");
    let (sources, tree) = input(&invalid);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn singleton_results_on_only_the_normal_branch_do_not_invent_an_exiting_value() {
    let source = "const func work(flag: bool): i32 { var values = [20, 20]\nedit left = values[if flag { 0 } else { return 99 }]\nedit right = values[1]\nleft += 1\nright += 1\nleft + right }\nconst ANSWER: (i32, i32) = (work(true), work(false))";
    assert_eq!(
        answer(source, "ANSWER"),
        Datum::aggregate(vec![Datum::integer(42), Datum::integer(99)])
    );
    let source = "func work(flag: bool): i32 { var values = [20, 22]\nread first = values[if flag { 0 } else { 1 }]\nvalues[0] = 42\nfirst }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
#[test]
fn zero_byte_elements_keep_distinct_logical_regions() {
    let source = "const func assign(edit left: void, edit right: void): i32 { left = ()\nright = ()\n42 }\nconst func work(): i32 { var values = [(), ()]\nassign(values[0], values[1]) }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn availability_and_access_share_the_typed_mixed_selection_path() {
    use crate::value_access::{AccessPath, IndexFact, Referent};
    use nocter_model::ValueProjection;
    let unit = accepted(
        "struct Cell { first: i32\nsecond: i32 }\nstruct Holder { values: [[Cell; 2]; 2] }\nfunc work(root: Holder, index: usize): i32 { root.values[index][1].first }",
    );
    let query = named(&unit, "work");
    let body = query.typed().body();
    let id = body
        .expressions()
        .iter()
        .find_map(|(id, e)| {
            matches!(
                e.kind,
                crate::value_access::ExpressionKind::Select {
                    action: crate::value_access::SelectionAction::Access,
                    ..
                }
            )
            .then_some(id)
        })
        .unwrap();
    let (local, path) = Referent::Evaluated(id)
        .projection_path(
            |id| body.places().get(id),
            |id| body.expressions().get(id),
            |id| body.blocks().get(id),
            IndexFact::Known,
            |_, operand| {
                body.expressions()
                    .get(operand)
                    .map(crate::value_access::Expression::index_fact)
            },
        )
        .unwrap();
    assert_eq!(local, body.locals().iter().next().unwrap().0);
    let field = |record, name| match super::support::record_field(&unit, record, name) {
        ValueProjection::Record(id) => ValueProjection::Record(id),
        _ => unreachable!(),
    };
    assert_eq!(
        path,
        AccessPath::new([
            field("Holder", "values"),
            ValueProjection::Array(IndexFact::Unknown),
            ValueProjection::Array(IndexFact::Known(1)),
            field("Cell", "first"),
        ])
    );
    assert!(crate::value_access::paths::concrete(&path).is_none());
}
