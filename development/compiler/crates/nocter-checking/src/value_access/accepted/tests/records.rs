//! Source-to-accepted-unit tests; no synthetic body or resource authority is assembled here.
use super::*;
use crate::value_access::{BoundaryAction, ExpressionKind};
use nocter_model::ValueInputOperation;
use std::fmt::Write;

use super::support::{record_field, source_case, steps};

#[test]
fn source_field_return_stages_the_exact_selected_value_before_sibling_cleanup() {
    let unit = accepted(&source_case("a15-record-field-return.nct"));
    let first = record_field(&unit, "Pair", "first");
    let second = record_field(&unit, "Pair", "second");
    let trace = steps(named(&unit, "first"));
    let selected = trace.iter().position(|step| matches!(step, Step::Move { source, payload, .. } if source.fields().fields() == [first] && payload.disposal() == Disposal::OwnedError)).unwrap();
    let returned = trace.iter().position(|step| matches!(step, Step::Move { destination, .. } if destination.holder() == Holder::Return)).unwrap();
    let disposed = trace.iter().position(|step| matches!(step, Step::Dispose { source, payload } if source.fields().fields() == [second] && payload.disposal() == Disposal::OwnedError)).unwrap();
    let released = trace.iter().position(|step| matches!(step, Step::ReleaseContainer { source, .. } if source.fields().is_root())).unwrap();
    assert!(selected < returned && returned < disposed && disposed < released);
    assert!(!trace.iter().any(
        |step| matches!(step, Step::Dispose { source, .. } if source.fields().fields() == [first])
    ));
}

#[test]
fn source_aggregate_and_component_copy_use_the_same_selected_boundary_operation() {
    for (file, function) in [
        ("a22-copy-record.nct", "copy"),
        ("a25-copy-record-component.nct", "read"),
    ] {
        let unit = accepted(&source_case(file));
        let query = named(&unit, function);
        assert!(
            query
                .typed()
                .body()
                .expressions()
                .iter()
                .any(|(_, expression)| matches!(
                    expression.kind,
                    ExpressionKind::Boundary {
                        action: BoundaryAction::CopyPayload,
                        ..
                    }
                ))
        );
        let callable = unit
            .program()
            .codes()
            .get(query.typed().source_owner())
            .unwrap();
        assert_eq!(
            unit.program()
                .parameters()
                .get(callable.parameters()[0])
                .unwrap()
                .operation(),
            ValueInputOperation::Read
        );
    }
}

#[test]
fn source_partial_drop_does_not_destroy_values_already_moved_into_other_bindings() {
    let unit = accepted(&source_case("a24-all-fields-moved-drop.nct"));
    let trace = steps(named(&unit, "extract"));
    let first = record_field(&unit, "Pair", "first");
    let second = record_field(&unit, "Pair", "second");
    for field in [first, second] {
        assert_eq!(trace.iter().filter(|step| matches!(step, Step::Move { source, .. } if source.fields().fields() == [field])).count(), 1);
        assert!(!trace.iter().any(|step| matches!(step, Step::Dispose { source, .. } if source.fields().fields() == [field])));
    }
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step, Step::ReleaseContainer { .. }))
            .count(),
        1
    );
}

#[test]
fn source_branch_replacement_retains_both_old_value_cleanup_alternatives() {
    let unit = accepted(&source_case("a20-record-branch-restore.nct"));
    let query = named(&unit, "run");
    let first = record_field(&unit, "Pair", "first");
    let mut cleanup = Vec::new();
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if matches!(event.operation, Operation::Assign { .. }) {
                for transition in query.access().resources().at(Location { point, action }) {
                    cleanup.push(transition.steps().iter().any(|step| matches!(step, Step::Dispose { source, .. } if source.fields().fields() == [first])));
                }
            }
        }
    }
    cleanup.sort_unstable();
    cleanup.dedup();
    assert_eq!(cleanup, [false, true]);
}

#[test]
fn record_header_changes_invalidate_the_existing_authority_without_relowering_on_bind() {
    let text = source_case("a15-record-field-return.nct");
    let (sources, tree) = input(&text);
    let query = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let (moved, moved_tree) = input(&format!("// source relocated\n{text}"));
    assert!(
        unit.bind(&query.bind(bound(&moved, &moved_tree)).unwrap())
            .is_ok()
    );
    let (changed, changed_tree) = input(&text.replace("first: error", "first: i32"));
    assert!(matches!(
        query.bind(bound(&changed, &changed_tree)),
        Err(declarations::Failure::ChangedHeader)
    ));
}

#[test]
fn external_record_update_retains_the_selected_field_in_its_effect() {
    let unit = accepted(&source_case("a31-edit-record-input.nct"));
    let first = record_field(&unit, "Pair", "first");
    let trace = steps(named(&unit, "replace"));
    assert!(trace.iter().any(|step| matches!(step, Step::ReplaceInput { fields, payload, .. } if fields.fields() == [first] && payload.disposal() == Disposal::OwnedError)));
}

#[test]
fn source_field_selection_and_cleanup_preserve_shared_deep_schemas() {
    const DEPTH: usize = 64;
    let mut text = "struct R0 {\n    value: error\n    other: error\n}\n".to_owned();
    for depth in 1..=DEPTH {
        writeln!(
            text,
            "struct R{depth} {{\n    left: R{}\n    right: R{}\n}}",
            depth - 1,
            depth - 1
        )
        .unwrap();
    }
    writeln!(
        text,
        "func deep(take root: R{DEPTH}): error {{ root{}.value^ }}",
        ".left".repeat(DEPTH)
    )
    .unwrap();
    let unit = accepted(&text);
    let query = named(&unit, "deep");
    // Prefix syntax does not create redundant selected places or expand the conceptual tree.
    assert_eq!(query.typed().body().places().len(), 1);
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
