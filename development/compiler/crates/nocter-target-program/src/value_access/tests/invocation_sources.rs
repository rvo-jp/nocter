//! Actual prepared values, including synthesized construction, own invocation payload selection.
use super::*;

#[test]
fn compound_updates_and_implicit_outcome_construction_retain_prepared_value_evidence() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/invocation-sources.nct"
    )));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let calculate = callable(graph.unit(), "calculate");
    let (_, caller) = graph
        .instances()
        .iter()
        .find(|(_, instance)| instance.key().code() == ValueCode::Declaration(calculate))
        .unwrap();
    assert_eq!(caller.calls().len(), 1);
    let call = caller.calls().values().next().unwrap();
    let argument = &call.inputs()[0].payload;
    assert!(matches!(
        graph.unit().program().data_schema(argument.data()),
        Some(nocter_declarations::value_access::PayloadSchema::Optional(
            _
        ))
    ));
    let source = graph.unit().value_sources(caller.key().code()).unwrap();
    let prepared = source.call(*caller.calls().keys().next().unwrap()).unwrap()[0].arguments[0];
    assert!(has_primary_construction(source, prepared));
}

fn has_primary_construction(
    graph: &nocter_checking::value_access::ValueSources,
    root: nocter_checking::value_access::ValueSourceId,
) -> bool {
    use nocter_checking::value_access::ValueSource;
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        match &graph.node(id).unwrap().value {
            ValueSource::Aggregate(fields)
                if fields
                    .iter()
                    .any(|(field, _)| *field == nocter_model::ValueField::Variant(true)) =>
            {
                return true;
            }
            ValueSource::Adapt { source } => pending.push(*source),
            ValueSource::Merge { inputs, .. } | ValueSource::Alternatives(inputs) => {
                pending.extend(inputs);
            }
            _ => {}
        }
    }
    false
}
