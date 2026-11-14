//! Static producer flow over the selected control graph, independent of owner availability.
//! A conditional's provisional type is not evidence that its named branch can execute.
use crate::value_access::{CallKind, ExpressionKind, Query, flow};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueApplication, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Producers = BTreeSet<FunctionWitness>;
pub(super) type Inputs = BTreeMap<nocter_model::ValueParameter, Producers>;
pub(super) struct Report {
    pub(super) returned: Vec<(FunctionWitness, AccessExpressionId)>,
    pub(super) applications: BTreeSet<ValueApplication>,
    pub(super) calls: BTreeMap<AccessExpressionId, super::relations::InvocationRecipe>,
    pub(super) creations: BTreeMap<nocter_model::ValueClosure, CreationRecipe>,
    /// Received or transported leaves retain their original relation, never a signature-derived
    /// concrete anchor. An uncommitted origin can therefore remain genuinely empty.
    pub(super) referenced_applications: BTreeSet<ValueApplication>,
}
use super::value_sources::ValueSourceSlot as Key;

mod creations;
mod facts;
use creations::Creations;
pub use creations::{CreationRecipe, ProducerShape};
use facts::{Facts, Store};
type State = BTreeMap<Key, Facts>;

pub(super) fn analyze(control: &flow::Control) -> Result<Report, flow::IntegrityError> {
    let query = &control.typed;
    let mut facts = Store::default();
    let aliases = flow::Aliases::prepare(query)?;
    let (initial, mut referenced_applications) = received_inputs(query, &mut facts)?;
    let mut entries = BTreeMap::from([(control.graph.entry(), initial)]);
    let mut pending = BTreeSet::from([control.graph.entry()]);
    let mut returned = BTreeSet::new();
    let mut applications = BTreeSet::new();
    let mut calls = BTreeMap::new();
    let mut creations = Creations::default();
    while let Some(point) = pending.pop_first() {
        let mut state = entries.get(&point).ok_or(INVALID)?.clone();
        let block = control.graph.points().get(point).ok_or(INVALID)?;
        for action in &block.actions {
            match action.operation {
                flow::Operation::Evaluate(id) => {
                    let producers =
                        evaluate(query, id, &state, &mut facts, &aliases, &mut creations)?;
                    state.insert(Key::Expression(id), producers);
                }
                flow::Operation::PrepareArgument {
                    call,
                    argument: index,
                } => {
                    let value =
                        prepared_argument(query, call, index, &state, &mut facts, &aliases)?;
                    state.insert(Key::Argument { call, index }, value);
                }
                flow::Operation::CommitCall(id) => {
                    commit(
                        query,
                        id,
                        &mut state,
                        &mut applications,
                        &mut calls,
                        &mut referenced_applications,
                        &mut facts,
                    )?;
                }
                flow::Operation::Forward { source, result, .. } => {
                    let value = forward(
                        query,
                        source,
                        expression_data(query, result)?,
                        &state,
                        &mut facts,
                        &aliases,
                    )?;
                    state.insert(Key::Expression(result), value);
                }
                flow::Operation::Bind {
                    local,
                    value: source,
                } => {
                    let value = forward(
                        query,
                        source,
                        query.body.locals.get(local).ok_or(INVALID)?.data,
                        &state,
                        &mut facts,
                        &aliases,
                    )?;
                    state.insert(Key::Local(local), value);
                }
                flow::Operation::Assign { target, value } => {
                    assign(query, target, value, &mut state, &mut facts, &aliases)?;
                }
                flow::Operation::StageReturn(id) => {
                    returned.extend(
                        value(query, id, &state, &mut facts, &aliases)?
                            .into_iter()
                            .map(|witness| (witness, id)),
                    );
                }
                // Code/type witness facts do not end when an owner is moved or dropped.
                // Initialization and access validation independently reject unavailable values.
                _ => {}
            }
        }
        for next in block.terminator.successors() {
            let changed = if let Some(entry) = entries.get_mut(&next) {
                join(entry, &state, &mut facts)
            } else {
                entries.insert(next, state.clone());
                true
            };
            if changed {
                pending.insert(next);
            }
        }
    }
    Ok(Report {
        returned: returned.into_iter().collect(),
        applications,
        calls,
        creations: creations.finish(query.code, facts),
        referenced_applications,
    })
}
fn prepared_argument(
    query: &Query,
    call: AccessExpressionId,
    index: usize,
    state: &State,
    store: &mut Store,
    aliases: &flow::Aliases,
) -> Result<Facts, flow::IntegrityError> {
    let ExpressionKind::Call { arguments, .. } =
        &query.body.expressions.get(call).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    let argument = arguments.get(index).ok_or(INVALID)?;
    shape(query, argument.value, state, false, store, aliases)
}
fn received_inputs(
    query: &Query,
    facts: &mut Store,
) -> Result<(State, BTreeSet<ValueApplication>), flow::IntegrityError> {
    let mut initial = State::new();
    let mut applications = BTreeSet::new();
    for (local, value) in query.body.locals.iter() {
        if value.origin.is_input() {
            let value = facts.received(
                query.program(),
                value.data,
                value.origin == crate::value_access::ir::LocalOrigin::Capture,
            )?;
            applications.extend(facts.applications(value));
            initial.insert(Key::Local(local), value);
        }
    }
    Ok((initial, applications))
}
fn assign(
    query: &Query,
    target: crate::value_access::Referent,
    source: AccessExpressionId,
    state: &mut State,
    store: &mut Store,
    aliases: &flow::Aliases,
) -> Result<(), flow::IntegrityError> {
    let facts = forward(
        query,
        source,
        query.body.referent_data(target).ok_or(INVALID)?,
        state,
        store,
        aliases,
    )?;
    if let Some(paths) = aliases.producer_paths(query, target) {
        let weak = paths.len() != 1;
        for (root, path) in paths {
            let (key, root) = producer_root(query, root)?;
            // This projects a possible element's data shape, never a numeric access receipt.
            let data = query
                .program()
                .project_data_by(root, &path, |(), _| true)
                .ok_or(INVALID)?;
            let replacement = store.retag(
                query.program(),
                facts,
                query.body.referent_data(target).ok_or(INVALID)?,
                data,
            )?;
            let original = state.get(&key).copied().unwrap_or_default();
            let updated = store.update(original, path.fields(), replacement, weak);
            state.insert(key, updated);
        }
        return Ok(());
    }
    if let crate::value_access::Referent::Place(selected) = target {
        let place = query.body.places.get(selected).ok_or(INVALID)?;
        let original = state
            .get(&Key::Local(place.local))
            .copied()
            .unwrap_or_default();
        let updated = store.update(original, place.fields.fields(), facts, false);
        state.insert(Key::Local(place.local), updated);
        return Ok(());
    }
    let projections = target
        .projections(
            |id| query.body.expressions.get(id),
            |id| query.body.blocks.get(id),
            crate::value_access::paths::ProjectionRoots::All,
            |_| 0_u64,
            |_, _| Some(0_u64),
        )
        .ok_or(INVALID)?;
    for (base, suffix) in projections {
        let crate::value_access::paths::ProjectionRoot::Place(base) = base else {
            continue;
        };
        let place = query.body.places.get(base).ok_or(INVALID)?;
        let path = place.fields.concat(&suffix);
        let original = state
            .get(&Key::Local(place.local))
            .copied()
            .unwrap_or_default();
        // Evaluated alternatives and array elements remain conservative summaries.
        let updated = store.update(original, path.fields(), facts, true);
        state.insert(Key::Local(place.local), updated);
    }
    Ok(())
}

fn commit(
    query: &Query,
    id: AccessExpressionId,
    state: &mut State,
    applications: &mut BTreeSet<ValueApplication>,
    calls: &mut BTreeMap<AccessExpressionId, super::relations::InvocationRecipe>,
    references: &mut BTreeSet<ValueApplication>,
    store: &mut Store,
) -> Result<(), flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    let ExpressionKind::Call {
        arguments, kind, ..
    } = &expression.kind
    else {
        return Err(INVALID);
    };
    let selected = super::applications::callee(query, id)?;
    let callees = match kind {
        CallKind::Direct => Producers::from([selected]),
        CallKind::Value => {
            arguments.first().ok_or(INVALID)?;
            store
                .producers(
                    *state
                        .get(&Key::Argument { call: id, index: 0 })
                        .ok_or(INVALID)?,
                )
                .into_iter()
                .map(|witness| {
                    super::relations::adapt(witness, selected.invocation).map_err(|()| INVALID)
                })
                .collect::<Result<Producers, _>>()?
        }
    };
    let invocation = calls
        .entry(id)
        .or_insert_with(|| super::relations::InvocationRecipe {
            callees: Producers::new(),
            inputs: Inputs::new(),
        });
    invocation.callees.extend(callees);
    let inputs = &mut invocation.inputs;
    for (index, argument) in arguments.iter().enumerate() {
        if let Some(binding) = &argument.binding {
            inputs.entry(binding.parameter).or_default().extend(
                store.producers(
                    *state
                        .get(&Key::Argument { call: id, index })
                        .ok_or(INVALID)?,
                ),
            );
        }
    }
    let Some(result) = super::applications::call_result(query, id)? else {
        return Ok(());
    };
    let Some(application) = result.created else {
        let facts = store.received(query.program(), result.data, true)?;
        references.extend(store.applications(facts));
        state.insert(Key::Expression(id), facts);
        return Ok(());
    };
    applications.insert(application);
    let Some(PayloadSchema::Function(witness)) = query.program().data_schema(result.data) else {
        return Err(INVALID);
    };
    state.insert(Key::Expression(id), store.single(witness));

    Ok(())
}

const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
use super::value_sources::root as producer_root;

fn join(target: &mut State, incoming: &State, store: &mut Store) -> bool {
    let mut changed = false;
    for (&key, producers) in incoming {
        let entry = target.entry(key).or_default();
        let joined = store.join(*entry, *producers);
        changed |= joined != *entry;
        *entry = joined;
    }
    changed
}
fn expression_data(
    query: &Query,
    id: AccessExpressionId,
) -> Result<nocter_model::TypeId, flow::IntegrityError> {
    super::value_sources::data(query, id)?.ok_or(INVALID)
}

fn forward(
    query: &Query,
    source: AccessExpressionId,
    target: nocter_model::TypeId,
    state: &State,
    store: &mut Store,
    aliases: &flow::Aliases,
) -> Result<Facts, flow::IntegrityError> {
    let expression = query.body.expressions.get(source).ok_or(INVALID)?;
    let Some(access) = expression.ty.access() else {
        return Ok(Facts::default());
    };
    let facts = shape(query, source, state, false, store, aliases)?;
    store.retag(query.program(), facts, access.data(), target)
}
fn value(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
    store: &mut Store,
    aliases: &flow::Aliases,
) -> Result<Producers, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    if expression.ty.access().is_none() {
        return Ok(Producers::new());
    }
    let facts = shape(query, id, state, false, store, aliases)?;
    Ok(store.producers(facts))
}
enum Projection {
    Boundary {
        source: nocter_model::TypeId,
        target: nocter_model::TypeId,
    },
    Select(nocter_model::FieldPath),
}
fn shape(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
    fresh: bool,
    store: &mut Store,
    aliases: &flow::Aliases,
) -> Result<Facts, flow::IntegrityError> {
    let mut current = id;
    let mut projections = Vec::new();
    let facts = loop {
        let expression = query.body.expressions.get(current).ok_or(INVALID)?;
        let Some(access) = expression.ty.access() else {
            break Facts::default();
        };
        if let ExpressionKind::Place(place_id)
        | ExpressionKind::Move(crate::value_access::Referent::Place(place_id)) = expression.kind
        {
            let place = query.body.places.get(place_id).ok_or(INVALID)?;
            if let Some(paths) =
                aliases.producer_paths(query, crate::value_access::Referent::Place(place_id))
            {
                let mut result = Facts::default();
                for (root, path) in paths {
                    let (key, root) = producer_root(query, root)?;
                    let data = query
                        .program()
                        .project_data_by(root, &path, |(), _| true)
                        .ok_or(INVALID)?;
                    let original = state.get(&key).copied().unwrap_or_default();
                    let selected = store.selected(original, path.fields());
                    let selected = store.retag(query.program(), selected, data, place.data)?;
                    result = store.join(result, selected);
                }
                break result;
            }
            break store.selected(
                state
                    .get(&Key::Local(place.local))
                    .copied()
                    .unwrap_or_default(),
                place.fields.fields(),
            );
        }
        if (!fresh || current != id)
            && let Some(facts) = state.get(&Key::Expression(current))
        {
            break *facts;
        }
        let next = match expression.kind {
            ExpressionKind::Boundary { source, .. }
            | ExpressionKind::AdaptFunction { source }
            | ExpressionKind::Move(crate::value_access::Referent::Evaluated(source)) => {
                projections.push(Projection::Boundary {
                    source: expression_data(query, source)?,
                    target: access.data(),
                });
                source
            }
            ExpressionKind::Index { base, .. } => {
                projections.push(Projection::Select(nocter_model::FieldPath::new([
                    nocter_model::ValueField::Array(0),
                ])));
                base
            }
            ExpressionKind::Select {
                source: base,
                ref fields,
                ..
            } => {
                projections.push(Projection::Select(fields.clone()));
                base
            }
            _ => {
                break state
                    .get(&Key::Expression(current))
                    .copied()
                    .unwrap_or_default();
            }
        };
        if next >= current {
            return Err(INVALID);
        }
        current = next;
    };
    projections
        .into_iter()
        .rev()
        .try_fold(facts, |facts, projection| match projection {
            Projection::Boundary { source, target } => {
                store.retag(query.program(), facts, source, target)
            }
            Projection::Select(fields) => Ok(store.selected(facts, fields.fields())),
        })
}
fn evaluate(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
    store: &mut Store,
    aliases: &flow::Aliases,
    creations: &mut Creations,
) -> Result<Facts, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    match &expression.kind {
        ExpressionKind::Closure { closure, capture } => {
            let captured = shape(query, *capture, state, false, store, aliases)?;
            creations.record(
                *closure,
                id,
                expression_data(query, *capture)?,
                captured,
                store,
            )?;
            let invocation = query
                .program()
                .closure(*closure)
                .ok_or(INVALID)?
                .contract()
                .invocation;
            Ok(store.single(FunctionWitness {
                origin: ValueFunctionOrigin::Closure(*closure),
                invocation,
            }))
        }
        ExpressionKind::Function(declaration) => Ok(store.single(FunctionWitness {
            origin: ValueFunctionOrigin::Declaration(*declaration),
            invocation: nocter_model::ValueInvocation::Read,
        })),
        ExpressionKind::Aggregate { fields } => {
            let data = expression_data(query, id)?;
            let mut children = BTreeMap::new();
            for field in fields {
                let target = query
                    .program()
                    .component(data, field.field)
                    .ok_or(INVALID)?
                    .data;
                children.insert(
                    field.field,
                    forward(query, field.value, target, state, store, aliases)?,
                );
            }
            if matches!(
                query.program().data_schema(data),
                Some(PayloadSchema::Array(_))
            ) {
                let element = children
                    .into_values()
                    .fold(Facts::default(), |all, next| store.join(all, next));
                Ok(store.array(element))
            } else {
                Ok(store.aggregate(children))
            }
        }
        ExpressionKind::Repeat { seed } => {
            let Some(PayloadSchema::Array(array)) =
                query.program().data_schema(expression_data(query, id)?)
            else {
                return Err(INVALID);
            };
            if array.is_empty() {
                Ok(Facts::default())
            } else {
                let element = forward(query, *seed, array.element(), state, store, aliases)?;
                Ok(store.array(element))
            }
        }
        ExpressionKind::Place(_)
        | ExpressionKind::Move(_)
        | ExpressionKind::Boundary { .. }
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::Index { .. }
        | ExpressionKind::Select { .. } => shape(query, id, state, true, store, aliases),
        _ => Ok(Facts::default()),
    }
}
