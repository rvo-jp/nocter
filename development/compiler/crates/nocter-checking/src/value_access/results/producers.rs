//! Static producer flow over the selected control graph, independent of owner availability.
//! A conditional's provisional type is not evidence that its named branch can execute.
use crate::value_access::{ExpressionKind, LocalOrigin, Query, flow};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{
    AccessExpressionId, AccessLocalId, AccessParameterId, ValueApplication, ValueFunctionOrigin,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Producers = BTreeSet<FunctionWitness>;
pub(super) type Inputs = BTreeMap<AccessParameterId, Producers>;
#[derive(Default)]
pub(super) struct Report {
    pub(super) returned: Vec<(FunctionWitness, AccessExpressionId)>,
    pub(super) applications: BTreeMap<ValueApplication, Inputs>,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Argument {
        call: AccessExpressionId,
        index: usize,
    },
}
type State = BTreeMap<Key, Producers>;

pub(super) fn analyze(control: &flow::Control) -> Result<Report, flow::IntegrityError> {
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
    let mut applications = BTreeMap::<ValueApplication, Inputs>::new();
    while let Some(point) = pending.pop_first() {
        let mut state = entries.get(&point).ok_or(INVALID)?.clone();
        let block = control.graph.points().get(point).ok_or(INVALID)?;
        for action in &block.actions {
            match action.operation {
                flow::Operation::Evaluate(id) => {
                    let producers = evaluate(query, id, &state)?;
                    state.insert(Key::Expression(id), producers);
                }
                flow::Operation::PrepareArgument {
                    call,
                    argument: index,
                } => {
                    let ExpressionKind::Call { arguments, .. } =
                        &query.body.expressions.get(call).ok_or(INVALID)?.kind
                    else {
                        return Err(INVALID);
                    };
                    let argument = arguments.get(index).ok_or(INVALID)?;
                    state.insert(
                        Key::Argument { call, index },
                        value(query, argument.value, &state)?,
                    );
                }
                flow::Operation::CommitCall(id) => {
                    commit(query, id, &mut state, &mut applications)?;
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
    Ok(Report {
        returned: returned.into_iter().collect(),
        applications,
    })
}
fn commit(
    query: &Query,
    id: AccessExpressionId,
    state: &mut State,
    applications: &mut BTreeMap<ValueApplication, Inputs>,
) -> Result<(), flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    let Some(PayloadSchema::Function(witness)) = expression
        .ty
        .access()
        .and_then(|a| query.program().data_schema(a.data()))
    else {
        return Ok(());
    };
    let ValueFunctionOrigin::Application(application) = witness.origin else {
        return Ok(());
    };
    let ExpressionKind::Call {
        target: ValueFunctionOrigin::Declaration(factory),
        arguments,
        ..
    } = &expression.kind
    else {
        return Err(INVALID);
    };
    if application.owner != query.source_owner()
        || application.call != id
        || application.callee != *factory
    {
        return Err(INVALID);
    }
    let inputs = applications.entry(application).or_default();
    for (index, argument) in arguments.iter().enumerate() {
        if let Some(binding) = &argument.binding {
            inputs.entry(binding.parameter).or_default().extend(
                state
                    .get(&Key::Argument { call: id, index })
                    .ok_or(INVALID)?,
            );
        }
    }
    state.insert(Key::Expression(id), BTreeSet::from([witness]));

    Ok(())
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
        ExpressionKind::Closure { closure, .. } => {
            let invocation = query
                .program()
                .closure(closure)
                .ok_or(INVALID)?
                .contract()
                .invocation;
            Ok(BTreeSet::from([FunctionWitness {
                origin: ValueFunctionOrigin::Closure(closure),
                invocation,
            }]))
        }
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
