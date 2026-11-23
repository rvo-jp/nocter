//! Choose stable prefix lifecycles for proved loop-carried numerical histories.
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
    let backedges = backedges(graph)?;
    let mut carried_edges = Vec::new();
    let mut locations = BTreeMap::<AccessExpressionId, BTreeSet<AccessFlowPointId>>::new();
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
                for &site in &transfers.proven {
                    locations.entry(site).or_default().insert(point);
                }
                reports.insert(Location { point, action }, transfers);
            }
        }
        for successor in body.terminator.successors() {
            if let Some(members) = backedges.get(&(point, successor)) {
                let mut edge = state.clone();
                edge.edge(typed, cells, &body.terminator, successor)?;
                carried_edges.push((members, edge.carried_prefixes(cells)));
            }
        }
    }
    let carried = carried_edges
        .into_iter()
        .flat_map(|(members, sites)| {
            sites.into_iter().filter(|site| {
                locations
                    .get(site)
                    .is_some_and(|points| points.iter().any(|point| members.contains(point)))
            })
        })
        .collect();
    let active = active_sites(&reports, &carried);
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

fn active_sites(
    reports: &BTreeMap<Location, Transfers>,
    carried: &BTreeSet<AccessExpressionId>,
) -> BTreeSet<AccessExpressionId> {
    let mut neighbors: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    let proven: BTreeSet<_> = reports
        .values()
        .flat_map(|report| report.proven.iter().copied())
        .collect();
    let mut pending: BTreeSet<_> = carried.intersection(&proven).copied().collect();
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

// A proved prefix crossing a backedge needs a stable lifecycle even if a later condition
// makes its next transfer unreachable. Reset epochs have already forgotten their sources.
fn backedges(
    graph: &Graph,
) -> Result<
    BTreeMap<(AccessFlowPointId, AccessFlowPointId), BTreeSet<AccessFlowPointId>>,
    IntegrityError,
> {
    let mut active = BTreeSet::from([graph.entry()]);
    let mut finished = BTreeSet::new();
    let mut stack = vec![(graph.entry(), 0)];
    let mut result = BTreeSet::new();
    while let Some((point, index)) = stack.last_mut() {
        let successors = graph
            .points()
            .get(*point)
            .ok_or(INVALID)?
            .terminator
            .successors();
        if let Some(&next) = successors.get(*index) {
            *index += 1;
            if active.contains(&next) {
                result.insert((*point, next));
            } else if !finished.contains(&next) {
                active.insert(next);
                stack.push((next, 0));
            }
        } else {
            let (point, _) = stack.pop().ok_or(INVALID)?;
            active.remove(&point);
            finished.insert(point);
        }
    }
    let mut predecessors = BTreeMap::<AccessFlowPointId, BTreeSet<AccessFlowPointId>>::new();
    for &point in &finished {
        for successor in graph
            .points()
            .get(point)
            .ok_or(INVALID)?
            .terminator
            .successors()
        {
            predecessors.entry(successor).or_default().insert(point);
        }
    }
    let mut loops = BTreeMap::new();
    for (tail, head) in result {
        let mut members = BTreeSet::from([head]);
        let mut pending = vec![tail];
        while let Some(point) = pending.pop() {
            if members.insert(point) {
                pending.extend(predecessors.get(&point).into_iter().flatten().copied());
            }
        }
        loops.insert((tail, head), members);
    }
    Ok(loops)
}
