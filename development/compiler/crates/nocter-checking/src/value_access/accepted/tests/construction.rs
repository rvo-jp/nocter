//! Actual source construction, component ownership and interrupted evaluation.
use super::support::{record_field, source_case, steps};
use super::*;
use crate::value_access::ExpressionKind;

#[test]
fn component_mapping_keeps_authored_order_and_completed_cleanup_uses_declaration_order() {
    let unit = accepted(&source_case("a33-record-value-order.nct"));
    let first = record_field(&unit, "Trio", "first");
    let second = record_field(&unit, "Trio", "second");
    let third = record_field(&unit, "Trio", "third");
    let trace = steps(named(&unit, "build"));
    let assemblies = trace
        .iter()
        .filter_map(|step| match step {
            Step::Assemble { fields, .. } => Some(fields),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(assemblies.len(), 1);
    assert_eq!(
        assemblies[0]
            .iter()
            .map(|(field, _)| *field)
            .collect::<Vec<_>>(),
        [third, first, second]
    );
    let cleanup = trace
        .iter()
        .filter_map(|step| match step {
            Step::Dispose { source, .. } => Some(source.fields().fields()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(cleanup, [&[third][..], &[second][..]]);
    assert!(!trace.iter().any(|step| matches!(step, Step::Construct { payload, .. } if payload.disposal() == Disposal::Aggregate)));
}

#[test]
fn interrupted_construction_cleans_components_in_reverse_production_order_without_assembly() {
    let unit = accepted(&source_case("a37-interrupted-record-return.nct"));
    let trace = steps(named(&unit, "build"));
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Assemble { .. }))
    );
    let produced = trace
        .iter()
        .filter_map(|step| match step {
            Step::Move {
                source,
                destination,
                ..
            } if matches!(source.holder(), Holder::Local(_))
                && matches!(destination.holder(), Holder::Expression(_)) =>
            {
                Some(destination.holder())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(produced.len(), 3);
    let cleaned = trace
        .iter()
        .filter_map(|step| match step {
            Step::Dispose { source, payload } if payload.disposal() == Disposal::OwnedError => {
                Some(source.holder())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let components = named(&unit, "build")
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, e)| match &e.kind {
            ExpressionKind::Aggregate { fields } => Some(fields),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        cleaned,
        [
            Holder::Expression(components[1].value),
            Holder::Expression(components[0].value)
        ]
    );
    let staged = trace.iter().position(|step| matches!(step, Step::Move { destination, .. } if destination.holder() == Holder::Return)).unwrap();
    let cleanup = trace
        .iter()
        .position(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError))
        .unwrap();
    assert!(staged < cleanup);
}

#[test]
fn nested_construction_transfers_component_values_without_duplicate_destruction() {
    let unit = accepted(&source_case("a34-nested-record-values.nct"));
    let trace = steps(named(&unit, "build"));
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step, Step::Assemble { .. }))
            .count(),
        2
    );
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Dispose { .. }))
    );
}

#[test]
fn scalar_component_is_copied_before_a_later_initializer_invokes_an_update() {
    let unit = accepted(&source_case("a41-record-component-snapshot.nct"));
    let trace = steps(named(&unit, "build"));
    let first = record_field(&unit, "Counts", "first");
    let source = trace
        .iter()
        .find_map(|step| match step {
            Step::Assemble { fields, .. } => fields
                .iter()
                .find(|(field, _)| *field == first)
                .map(|(_, source)| source.holder()),
            _ => None,
        })
        .unwrap();
    let constructed = trace
        .iter()
        .position(
            |step| matches!(step, Step::Construct { destination, .. } if *destination == source),
        )
        .unwrap();
    let invoked = trace
        .iter()
        .position(|step| matches!(step, Step::Receive { .. }))
        .unwrap();
    assert!(constructed < invoked);
}
