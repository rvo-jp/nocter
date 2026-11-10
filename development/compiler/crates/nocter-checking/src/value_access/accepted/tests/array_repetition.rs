//! Source repetition, resource transitions and compact required-value publication.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
use std::num::NonZeroU64;
#[test]
fn zero_one_many_and_large_repetitions_evaluate_the_seed_once() {
    for count in [0, 1, 3, u64::MAX] {
        let source = format!(
            "const func seed(edit calls: i32): i32 {{ calls += 1\n42 }}\nconst func work(): ([i32; {count}], i32) {{ var calls = 0\nlet values = [seed(calls); {count}]\n(values, calls) }}\nconst ANSWER: ([i32; {count}], i32) = work()"
        );
        let expected = if let Some(count) = NonZeroU64::new(count) {
            Datum::repeated(Datum::integer(42), count)
        } else {
            Datum::aggregate(vec![])
        };
        assert_eq!(
            answer(&source, "ANSWER"),
            Datum::aggregate(vec![expected, Datum::integer(1)])
        );
    }
}
#[test]
fn noncopy_seeds_move_at_zero_or_one_but_cannot_be_duplicated() {
    for count in [0, 1] {
        let source = format!(
            "struct Item {{ value: i32 }}\nconst func work(): [Item; {count}] {{ let item = Item {{ value: 42 }}\n[item^; {count}] }}\nconst ANSWER: [Item; {count}] = work()"
        );
        let fields = if count == 0 {
            vec![]
        } else {
            vec![Datum::aggregate(vec![Datum::integer(42)])]
        };
        assert_eq!(answer(&source, "ANSWER"), Datum::aggregate(fields));
        accepted(&format!(
            "func work(take value: error): void {{ let values = [value^; {count}] }}"
        ));
        let invalid = format!(
            "struct Item {{ value: i32 }}\nfunc work(take value: Item): void {{ let values = [value^; {count}]\nlet _ = value^ }}"
        );
        let (sources, tree) = input(&invalid);
        assert!(matches!(
            SourceUnit::prepare(bound(&sources, &tree)),
            Err(SourceFailure::Checking(Failure::Flow { .. }))
        ));
    }
}
#[test]
fn repeated_constants_publish_compactly_and_nested_indices_reuse_their_data() {
    let source = "const VALUES: [[i32; 18446744073709551615]; 18446744073709551615] = [[21; 18446744073709551615]; 18446744073709551615]\nconst ANSWER: i32 = VALUES[18446744073709551614][0] * 2";
    let unit = accepted(source);
    let values = unit
        .program()
        .resolve_constant(unit.program().symbols().get("VALUES").unwrap())
        .unwrap();
    let frozen = unit.constant_value(values).unwrap();
    assert_eq!(frozen.nodes().len(), 3);
    assert!(
        matches!(frozen.node(), nocter_model::FrozenNode::Repeated { length, replacements, .. } if length.get() == u64::MAX && replacements.is_empty())
    );
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn copy_seeds_remain_available_and_whole_array_replacement_preserves_snapshots() {
    let source = "const func replace(edit values: [i32; 2]): void { values = [1, 1] }\nconst func work(): i32 { let seed = 20\nvar values = [seed; 2]\nlet saved = values\nreplace(values)\nseed + saved[0] + values[0] + values[1] }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn interrupted_seed_has_no_repetition_value_or_resource_owner() {
    let source = "const func work(): i32 { let values: [i32; 3] = [if true { return 42 } else { 0 }; 3]\n0 }\nconst ANSWER: i32 = work()";
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn zero_count_disposes_its_seed_before_constructing_the_empty_array() {
    use super::support::steps;
    use crate::value_access::ExpressionKind;
    let unit = accepted(
        "struct Item { value: i32 }\nfunc work(take item: Item): [Item; 0] { [item^; 0] }",
    );
    let query = named(&unit, "work");
    let (repeat, seed) = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| match expression.kind {
            ExpressionKind::Repeat { seed } => Some((id, seed)),
            _ => None,
        })
        .unwrap();
    let trace = steps(query);
    let disposal = trace.iter().position(|step| matches!(step, Step::Dispose { source, .. } if source.holder() == Holder::Expression(seed))).unwrap();
    let assembly = trace.iter().position(|step| matches!(step, Step::Assemble { destination, fields, .. } if *destination == Holder::Expression(repeat) && fields.is_empty())).unwrap();
    assert!(disposal < assembly);
}

#[test]
fn maximum_count_fits_a_small_working_storage_budget() {
    let (sources, tree) = input(
        "const VALUES: [i32; 18446744073709551615] = [42; 18446744073709551615]\nconst ANSWER: i32 = VALUES[18446744073709551614]",
    );
    let limits = nocter_constant_evaluation::state::Limits::new(
        nocter_constant_evaluation::CompileTimeEvaluationLimits::default(),
        NonZeroU64::new(256).unwrap(),
    );
    let compiled = SourceUnit::prepare_with_limits(bound(&sources, &tree), limits).unwrap();
    let unit = compiled.unit();
    let answer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&Datum::integer(42).freeze())
    );
}
#[test]
fn repeated_constant_consumers_invalidate_when_the_seed_supplier_changes() {
    let source = "const SEED: i32 = 20\nconst VALUES: [i32; 2] = [SEED; 2]\nconst ANSWER: i32 = VALUES[0] + VALUES[1] + 2\nfunc run(): i32 { ANSWER }";
    let (sources, tree) = input(source);
    let compiled = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    let unit = compiled.unit();
    let query = named(unit, "run").typed();
    let edited = source.replace("SEED: i32 = 20", "SEED: i32 = 21");
    let (next_sources, next_tree) = input(&edited);
    let next = compiled
        .declarations()
        .bind(bound(&next_sources, &next_tree))
        .unwrap();
    assert!(matches!(
        query.bind(&next),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    assert_eq!(answer(&edited, "ANSWER"), Datum::integer(44));
}
