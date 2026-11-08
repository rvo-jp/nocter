//! Static producer flow over the selected control graph, independent of owner availability.
//! A conditional's provisional type is not evidence that its named branch can execute.
use crate::value_access::{ExpressionKind, LocalOrigin, Query, flow};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, AccessLocalId, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

type Producers = BTreeSet<FunctionWitness>;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
}
type State = BTreeMap<Key, Producers>;

pub(super) fn returns(
    control: &flow::Control,
) -> Result<Vec<(FunctionWitness, AccessExpressionId)>, flow::IntegrityError> {
    let query = &control.typed;
    let mut initial = State::new();
    for (local, value) in query.body.locals.iter() {
        if matches!(value.origin, LocalOrigin::Parameter(_))
            && let Some(PayloadSchema::Function(witness)) = query.program().data_schema(value.data)
        {
            initial.insert(Key::Local(local), BTreeSet::from([witness]));
        }
    }
    let mut entries = BTreeMap::from([(control.graph.entry(), initial)]);
    let mut pending = BTreeSet::from([control.graph.entry()]);
    let mut returned = BTreeSet::new();
    while let Some(point) = pending.pop_first() {
        let mut state = entries.get(&point).ok_or(INVALID)?.clone();
        let block = control.graph.points().get(point).ok_or(INVALID)?;
        for action in &block.actions {
            match action.operation {
                flow::Operation::Evaluate(id) => {
                    let producers = evaluate(query, id, &state)?;
                    state.insert(Key::Expression(id), producers);
                }
                flow::Operation::CommitCall(id) => {
                    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
                    if let ExpressionKind::Call {
                        target: ValueFunctionOrigin::Declaration(factory),
                        ..
                    } = expression.kind
                        && query
                            .program()
                            .callables()
                            .get(factory)
                            .ok_or(INVALID)?
                            .result_contract()
                            .is_some()
                    {
                        let Some(PayloadSchema::Function(witness)) = expression
                            .ty
                            .access()
                            .and_then(|a| query.program().data_schema(a.data()))
                        else {
                            return Err(INVALID);
                        };
                        state.insert(
                            Key::Expression(id),
                            BTreeSet::from([FunctionWitness {
                                origin: ValueFunctionOrigin::Result(factory),
                                invocation: witness.invocation,
                            }]),
                        );
                    }
                }
                flow::Operation::Forward { source, result } => {
                    let producers = value(query, source, &state)?;
                    state.insert(Key::Expression(result), producers);
                }
                flow::Operation::Bind {
                    local,
                    value: source,
                } => {
                    let producers = value(query, source, &state)?;
                    state.insert(Key::Local(local), producers);
                }
                flow::Operation::Assign {
                    target,
                    value: source,
                } => {
                    let place = query.body.places.get(target).ok_or(INVALID)?;
                    if place.fields.is_root() {
                        let producers = value(query, source, &state)?;
                        state.insert(Key::Local(place.local), producers);
                    }
                }
                flow::Operation::StageReturn(id) => {
                    returned.extend(
                        value(query, id, &state)?
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
                join(entry, &state)
            } else {
                entries.insert(next, state.clone());
                true
            };
            if changed {
                pending.insert(next);
            }
        }
    }
    Ok(returned.into_iter().collect())
}
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
fn join(target: &mut State, incoming: &State) -> bool {
    let mut changed = false;
    for (&key, producers) in incoming {
        let existing = target.entry(key).or_default();
        let before = existing.len();
        existing.extend(producers);
        changed |= existing.len() != before;
    }
    changed
}
fn value(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
) -> Result<Producers, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    if !expression.ty.access().is_some_and(|a| {
        matches!(
            query.program().data_schema(a.data()),
            Some(PayloadSchema::Function(_))
        )
    }) {
        return Ok(Producers::new());
    }
    match expression.kind {
        ExpressionKind::Place(place) | ExpressionKind::Move(place) => {
            let place = query.body.places.get(place).ok_or(INVALID)?;
            Ok(state
                .get(&Key::Local(place.local))
                .cloned()
                .unwrap_or_default())
        }
        _ => Ok(state.get(&Key::Expression(id)).cloned().unwrap_or_default()),
    }
}
fn evaluate(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
) -> Result<Producers, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    match expression.kind {
        ExpressionKind::Function(declaration) => Ok(BTreeSet::from([FunctionWitness {
            origin: ValueFunctionOrigin::Declaration(declaration),
            invocation: nocter_model::ValueInvocation::Read,
        }])),
        ExpressionKind::Place(_) | ExpressionKind::Move(_) => value(query, id, state),
        ExpressionKind::Boundary { source, .. } => value(query, source, state),
        ExpressionKind::AdaptFunction { source } => {
            let Some(PayloadSchema::Function(view)) = expression
                .ty
                .access()
                .and_then(|a| query.program().data_schema(a.data()))
            else {
                return Err(INVALID);
            };
            Ok(value(query, source, state)?
                .into_iter()
                .map(|w| FunctionWitness {
                    origin: w.origin,
                    invocation: view.invocation,
                })
                .collect())
        }
        _ => Ok(Producers::new()),
    }
}
