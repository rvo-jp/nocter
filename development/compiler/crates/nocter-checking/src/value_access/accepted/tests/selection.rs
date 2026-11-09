//! Source-to-unit evidence for computed fields, selected operations and temporary remainders.
use super::support::{record_field, source_case, steps};
use super::*;
use crate::value_access::{ExpressionKind, Rule, SelectionAction};
use nocter_model::{AccessExpressionId, BuiltinType};
use std::fmt::Write;

fn selection(query: &flow::Query) -> (AccessExpressionId, AccessExpressionId, SelectionAction) {
    let selected = query
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(id, expression)| {
            if let ExpressionKind::Select { source, action, .. } = expression.kind {
                Some((id, source, action))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    selected[0]
}

#[test]
fn call_result_moves_one_field_before_return_staging_and_sibling_cleanup() {
    let unit = accepted(&source_case("a44-call-result-field.nct"));
    let query = named(&unit, "run");
    let (_, source, action) = selection(query);
    assert_eq!(action, SelectionAction::MovePayload);
    let first = record_field(&unit, "Pair", "first");
    let second = record_field(&unit, "Pair", "second");
    let trace = steps(query);
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step, Step::Receive { call, .. } if *call == source))
            .count(),
        1
    );
    let extracted = trace.iter().position(|step| matches!(step, Step::Move { source: place, .. } if place.holder() == Holder::Expression(source) && place.fields().fields() == [first])).unwrap();
    let staged = trace.iter().position(|step| matches!(step, Step::Move { destination, .. } if destination.holder() == Holder::Return)).unwrap();
    let cleanup = trace.iter().position(|step| matches!(step, Step::Dispose { source: place, .. } if place.holder() == Holder::Expression(source) && place.fields().fields() == [second])).unwrap();
    assert!(extracted < staged && staged < cleanup);
    assert!(!trace.iter().any(|step| matches!(step, Step::Dispose { source: place, .. } if place.fields().fields() == [first])));
}

#[test]
fn copy_component_preserves_the_complete_temporary_for_schema_cleanup() {
    for case in [
        "a45-literal-copy-field.nct",
        "a58-nominal-copy-result-field.nct",
    ] {
        let unit = accepted(&source_case(case));
        let query = named(&unit, "run");
        let (result, source, action) = selection(query);
        assert_eq!(action, SelectionAction::CopyPayload);
        let trace = steps(query);
        let copied = trace.iter().position(|step| matches!(step, Step::Construct { destination, .. } if *destination == Holder::Expression(result))).unwrap();
        let cleanup = trace.iter().position(|step| matches!(step, Step::Dispose { source: place, payload } if place.holder() == Holder::Expression(source) && place.fields().is_root() && payload.disposal() == Disposal::Record)).unwrap();
        assert!(copied < cleanup);
        assert!(!trace.iter().any(|step| matches!(step, Step::Move { source: place, .. } if place.holder() == Holder::Expression(source))));
        assert!(
            !trace
                .iter()
                .any(|step| matches!(step, Step::ReleaseContainer { .. }))
        );
    }
}

#[test]
fn read_supply_retains_the_selected_owner_and_its_source_remainder_until_frame_cleanup() {
    let unit = accepted(&source_case("a50-read-computed-field.nct"));
    let query = named(&unit, "run");
    let (_, source, _) = selection(query);
    let second = record_field(&unit, "Pair", "second");
    let trace = steps(query);
    let received = trace
        .iter()
        .rposition(|step| matches!(step, Step::Receive { .. }))
        .unwrap();
    let selected_cleanup = trace.iter().position(|step| matches!(step, Step::Dispose { source: place, payload } if matches!(place.holder(), Holder::Temporary { .. }) && payload.disposal() == Disposal::OwnedError)).unwrap();
    let remainder = trace.iter().position(|step| matches!(step, Step::Dispose { source: place, .. } if place.holder() == Holder::Expression(source) && place.fields().fields() == [second])).unwrap();
    assert!(received < selected_cleanup && selected_cleanup < remainder);
}

#[test]
fn interrupted_sources_keep_result_types_without_materializing_selected_values() {
    for case in [
        "a48-interrupted-call-field.nct",
        "a49-interrupted-literal-field.nct",
        "a56-interrupted-conditional-field.nct",
    ] {
        let unit = accepted(&source_case(case));
        let query = named(&unit, "run");
        let (result, _, _) = selection(query);
        let ty = query.typed().body().expressions().get(result).unwrap().ty;
        assert!(!ty.may_complete(), "{case}");
        assert_eq!(
            ty.access().unwrap().data(),
            unit.program().data_type(BuiltinType::Error).unwrap()
        );
        for (point, block) in query.graph().points().iter() {
            for (action, event) in block.actions.iter().enumerate() {
                if event.operation == Operation::Evaluate(result) {
                    assert!(
                        query
                            .access()
                            .resources()
                            .at(Location { point, action })
                            .next()
                            .is_none(),
                        "{case}"
                    );
                }
            }
        }
        assert!(
            !steps(query)
                .iter()
                .any(|step| matches!(step, Step::Receive { .. } | Step::Assemble { .. }))
        );
    }
}

#[test]
fn known_shapes_report_the_exact_unknown_field_and_containing_data() {
    for (case, data, field) in [
        ("r41-interrupted-call-unknown-field.nct", "Pair", "missing"),
        (
            "r42-interrupted-literal-unknown-field.nct",
            "Pair",
            "missing",
        ),
        ("r35-scalar-result-field.nct", "i32", "first"),
    ] {
        let text = source_case(case);
        let (sources, tree) = input(&text);
        let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = declarations.bind(bound(&sources, &tree)).unwrap();
        let Err(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        }) = Unit::prepare(&context)
        else {
            panic!("{case}");
        };
        assert_eq!(diagnostic.rule(), Rule::UnknownField);
        assert!(diagnostic.source().message().contains(data));
        assert!(diagnostic.source().message().contains(field));
        assert_eq!(
            context
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some(field)
        );
    }
}

#[test]
fn deeply_shared_computed_selection_freezes_one_path_and_only_cleans_its_frontier() {
    const DEPTH: usize = 64;
    let mut text = "struct R0 { value: error\nother: error }\n".to_owned();
    for depth in 1..=DEPTH {
        writeln!(
            text,
            "struct R{depth} {{ left: R{}\nright: R{} }}",
            depth - 1,
            depth - 1
        )
        .unwrap();
    }
    writeln!(
        text,
        "func identity(take root: R{DEPTH}): R{DEPTH} {{ root^ }}"
    )
    .unwrap();
    writeln!(
        text,
        "func run(take root: R{DEPTH}): error {{ identity(root^){}.value }}",
        ".left".repeat(DEPTH)
    )
    .unwrap();
    let unit = accepted(&text);
    let query = named(&unit, "run");
    let (result, _, _) = selection(query);
    let ExpressionKind::Select { fields, .. } =
        &query.typed().body().expressions().get(result).unwrap().kind
    else {
        panic!("selection");
    };
    assert_eq!(fields.fields().len(), DEPTH + 1);
    assert_eq!(query.typed().body().places().len(), 1); // The input operand, not computed prefixes.
    let trace = steps(query);
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step, Step::Dispose { .. }))
            .count(),
        DEPTH + 1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step, Step::ReleaseContainer { .. }))
            .count(),
        DEPTH + 1
    );
}
