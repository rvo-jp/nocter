//! Choose prefix lifecycles only when a proved transfer site can execute again in the same epoch.
use super::{Cells, INVALID, State, Transfers};
use crate::value_access::{
    ArgumentAction, ExpressionKind, Query as TypedQuery, Referent,
    flow::{
        Graph, IntegrityError, Location, Operation,
        resources::{ArrayFrontier, Place},
    },
};
use nocter_model::{AccessExpressionId, AccessFlowPointId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Plan {
    pub(super) prefixes: BTreeMap<Location, BTreeMap<AccessExpressionId, ArrayFrontier>>,
    pub(super) available: BTreeMap<Location, Place>,
}

fn prepared(typed: &TypedQuery, operation: &Operation) -> Option<AccessExpressionId> {
    let Operation::PrepareArgument { call, argument } = *operation else {
        return None;
    };
    let ExpressionKind::Call { ref arguments, .. } = typed.body().expressions().get(call)?.kind
    else {
        return None;
    };
    match arguments.get(argument)?.action {
        ArgumentAction::ReserveRead(Referent::Evaluated(id))
        | ArgumentAction::ReserveUpdate(Referent::Evaluated(id))
        | ArgumentAction::ReserveTransfer {
            place: Referent::Evaluated(id),
            ..
        }
        | ArgumentAction::CopyPlace(Referent::Evaluated(id)) => Some(id),
        _ => None,
    }
}

pub(super) fn prepare(
    typed: &TypedQuery,
    graph: &Graph,
    cells: &Cells,
    entries: &BTreeMap<AccessFlowPointId, State>,
) -> Result<Plan, IntegrityError> {
    let mut reports: BTreeMap<Location, Transfers> = BTreeMap::new();
    let mut available = BTreeMap::new();
    for (&point, entry) in entries {
        let body = graph.points().get(point).ok_or(INVALID)?;
        let mut state = entry.clone();
        for (action, item) in body.actions.iter().enumerate() {
            let at = Location { point, action };
            if let Some(selection) = prepared(typed, &item.operation)
                && let Some(array) = state.available(cells, selection)
            {
                available.insert(at, array);
            }
            let transfers = state.action(typed, cells, &item.operation)?;
            if !transfers.proven.is_empty() {
                reports.insert(Location { point, action }, transfers);
            }
        }
    }
    let active = active_sites(&reports);
    let prefixes = reports
        .into_iter()
        .filter_map(|(location, report)| {
            let bindings: BTreeMap<_, _> = report
                .proven
                .into_iter()
                .filter_map(|selection| {
                    let frontier = cells.histories.frontier(selection)?;
                    active.contains(&selection).then_some((selection, frontier))
                })
                .collect();
            (!bindings.is_empty()).then_some((location, bindings))
        })
        .collect();
    Ok(Plan {
        prefixes,
        available,
    })
}

fn active_sites(reports: &BTreeMap<Location, Transfers>) -> BTreeSet<AccessExpressionId> {
    let mut neighbors: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    let mut pending = BTreeSet::new();
    for report in reports.values() {
        pending.extend(&report.repeated);
        for (&site, peers) in &report.peers {
            neighbors.entry(site).or_default().extend(peers);
            for &peer in peers {
                neighbors.entry(peer).or_default().insert(site);
            }
        }
    }
    let mut active = BTreeSet::new();
    while let Some(site) = pending.pop_first() {
        if active.insert(site)
            && let Some(peers) = neighbors.get(&site)
        {
            pending.extend(peers.iter().filter(|peer| !active.contains(peer)));
        }
    }
    active
}
