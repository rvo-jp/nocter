//! Choose prefix lifecycles only when a proved transfer site can execute again in the same epoch.
use super::{Cells, INVALID, State, Transfers};
use crate::value_access::{
    Query as TypedQuery,
    flow::{Graph, IntegrityError, Location, resources::ArrayFrontier},
};
use nocter_model::{AccessExpressionId, AccessFlowPointId};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn prepare(
    typed: &TypedQuery,
    graph: &Graph,
    cells: &Cells,
    entries: &BTreeMap<AccessFlowPointId, State>,
) -> Result<BTreeMap<Location, BTreeMap<AccessExpressionId, ArrayFrontier>>, IntegrityError> {
    let mut reports: BTreeMap<Location, Transfers> = BTreeMap::new();
    for (&point, entry) in entries {
        let body = graph.points().get(point).ok_or(INVALID)?;
        let mut state = entry.clone();
        for (action, item) in body.actions.iter().enumerate() {
            let transfers = state.action(typed, cells, &item.operation)?;
            if !transfers.proven.is_empty() {
                reports.insert(Location { point, action }, transfers);
            }
        }
    }
    let active: BTreeSet<_> = reports
        .values()
        .flat_map(|report| &report.repeated)
        .filter_map(|&selection| cells.histories.frontier(selection))
        .collect();
    Ok(reports
        .into_iter()
        .filter_map(|(location, report)| {
            let bindings: BTreeMap<_, _> = report
                .proven
                .into_iter()
                .filter_map(|selection| {
                    let frontier = cells.histories.frontier(selection)?;
                    active.contains(&frontier).then_some((selection, frontier))
                })
                .collect();
            (!bindings.is_empty()).then_some((location, bindings))
        })
        .collect())
}
