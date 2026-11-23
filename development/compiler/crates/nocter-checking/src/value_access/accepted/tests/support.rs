//! Shared source-fixture and semantic-transition projections for accepted-unit tests.
use super::*;

pub(super) fn source_case(name: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/value-bindings")
            .join(name),
    )
    .unwrap()
}
pub(super) fn steps(query: &flow::Query) -> Vec<Step> {
    query
        .graph()
        .points()
        .iter()
        .flat_map(|(point, block)| {
            (0..block.actions.len()).flat_map(move |action| {
                query
                    .access()
                    .resources()
                    .at(Location { point, action })
                    .flat_map(|transition| transition.steps().iter().cloned())
            })
        })
        .collect()
}
pub(super) fn record_field(unit: &Unit, record: &str, name: &str) -> nocter_model::ValueField {
    let symbols = unit.program().symbols();
    unit.program()
        .record(
            unit.program()
                .resolve_nominal(symbols.get(record).unwrap())
                .unwrap(),
        )
        .unwrap()
        .resolve_field(symbols.get(name).unwrap())
        .unwrap()
        .into()
}
