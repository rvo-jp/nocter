//! Static producer flow over the selected control graph, independent of owner availability.
//! A conditional's provisional type is not evidence that its named branch can execute.
use crate::value_access::{ExpressionKind, Query, flow};
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
mod facts;
use facts::Facts;
type State = BTreeMap<Key, Facts>;

pub(super) fn analyze(control: &flow::Control) -> Result<Report, flow::IntegrityError> {
    let query = &control.typed;
    let mut initial = State::new();
    for (local, value) in query.body.locals.iter() {
        if value.origin.is_input() {
            initial.insert(
                Key::Local(local),
                Facts::received(query.program(), value.data)?,
            );
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
                        shape(query, argument.value, &state, false)?,
                    );
                }
                flow::Operation::CommitCall(id) => {
                    commit(query, id, &mut state, &mut applications)?;
                }
                flow::Operation::Forward { source, result } => {
                    let facts = forward(query, source, expression_data(query, result)?, &state)?;
                    state.insert(Key::Expression(result), facts);
                }
                flow::Operation::Bind {
                    local,
                    value: source,
                } => {
                    let facts = forward(
                        query,
                        source,
                        query.body.locals.get(local).ok_or(INVALID)?.data,
                        &state,
                    )?;
                    state.insert(Key::Local(local), facts);
                }
                flow::Operation::Assign { target, value } => {
                    assign(query, target, value, &mut state)?;
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
fn assign(
    query: &Query,
    target: crate::value_access::Referent,
    source: AccessExpressionId,
    state: &mut State,
) -> Result<(), flow::IntegrityError> {
    let Some(base) = target.static_base(|id| query.body.expressions.get(id)) else {
        return Ok(());
    };
    let place = query.body.places.get(base).ok_or(INVALID)?;
    let facts = forward(
        query,
        source,
        query.body.referent_data(target).ok_or(INVALID)?,
        state,
    )?;
    if matches!(target, crate::value_access::Referent::Place(selected) if query.body.places.get(selected).is_some_and(|place| place.fields.is_root()))
    {
        state.insert(Key::Local(place.local), facts);
    } else {
        // Homogeneous/field updates preserve the selected leaf type. Keep possible actual
        // producers without guessing an evaluated index or maintaining a second alias ledger.
        state
            .entry(Key::Local(place.local))
            .or_default()
            .join(&facts);
    }
    Ok(())
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
    if application.owner != query.code() || application.call != id || application.callee != *factory
    {
        return Err(INVALID);
    }
    let inputs = applications.entry(application).or_default();
    for (index, argument) in arguments.iter().enumerate() {
        if let Some(binding) = &argument.binding {
            inputs.entry(binding.parameter).or_default().extend(
                state
                    .get(&Key::Argument { call: id, index })
                    .ok_or(INVALID)?
                    .producers(expression_data(query, argument.value)?),
            );
        }
    }
    state.insert(
        Key::Expression(id),
        Facts::single(expression_data(query, id)?, witness),
    );

    Ok(())
}

const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
fn join(target: &mut State, incoming: &State) -> bool {
    let mut changed = false;
    for (&key, producers) in incoming {
        changed |= target.entry(key).or_default().join(producers);
    }
    changed
}
fn expression_data(
    query: &Query,
    id: AccessExpressionId,
) -> Result<nocter_model::TypeId, flow::IntegrityError> {
    query
        .body
        .expressions
        .get(id)
        .and_then(|expression| expression.ty.access())
        .map(nocter_model::ValueAccess::data)
        .ok_or(INVALID)
}
fn forward(
    query: &Query,
    source: AccessExpressionId,
    target: nocter_model::TypeId,
    state: &State,
) -> Result<Facts, flow::IntegrityError> {
    let expression = query.body.expressions.get(source).ok_or(INVALID)?;
    let Some(access) = expression.ty.access() else {
        return Ok(Facts::default());
    };
    shape(query, source, state, false)?.retag(query.program(), access.data(), target)
}
fn value(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
) -> Result<Producers, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    let Some(access) = expression.ty.access() else {
        return Ok(Producers::new());
    };
    Ok(shape(query, id, state, false)?.producers(access.data()))
}
enum Projection {
    Boundary {
        source: nocter_model::TypeId,
        target: nocter_model::TypeId,
    },
    Select(nocter_model::TypeId),
}
fn shape(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
    fresh: bool,
) -> Result<Facts, flow::IntegrityError> {
    let mut current = id;
    let mut projections = Vec::new();
    let facts = loop {
        let expression = query.body.expressions.get(current).ok_or(INVALID)?;
        let Some(access) = expression.ty.access() else {
            break Facts::default();
        };
        if let ExpressionKind::Place(place)
        | ExpressionKind::Move(crate::value_access::Referent::Place(place)) = expression.kind
        {
            let place = query.body.places.get(place).ok_or(INVALID)?;
            break state
                .get(&Key::Local(place.local))
                .cloned()
                .unwrap_or_default()
                .selected(query.program(), place.data)?;
        }
        if (!fresh || current != id)
            && let Some(facts) = state.get(&Key::Expression(current))
        {
            break facts.clone();
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
            ExpressionKind::Index { base, .. } | ExpressionKind::Select { source: base, .. } => {
                projections.push(Projection::Select(access.data()));
                base
            }
            _ => {
                break state
                    .get(&Key::Expression(current))
                    .cloned()
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
            Projection::Boundary { source, target } => facts.retag(query.program(), source, target),
            Projection::Select(data) => facts.selected(query.program(), data),
        })
}
fn evaluate(
    query: &Query,
    id: AccessExpressionId,
    state: &State,
) -> Result<Facts, flow::IntegrityError> {
    let expression = query.body.expressions.get(id).ok_or(INVALID)?;
    match &expression.kind {
        ExpressionKind::Closure { closure, .. } => {
            let invocation = query
                .program()
                .closure(*closure)
                .ok_or(INVALID)?
                .contract()
                .invocation;
            Ok(Facts::single(
                expression_data(query, id)?,
                FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(*closure),
                    invocation,
                },
            ))
        }
        ExpressionKind::Function(declaration) => Ok(Facts::single(
            expression_data(query, id)?,
            FunctionWitness {
                origin: ValueFunctionOrigin::Declaration(*declaration),
                invocation: nocter_model::ValueInvocation::Read,
            },
        )),
        ExpressionKind::Aggregate { fields } => {
            let data = expression_data(query, id)?;
            let mut facts = Facts::default();
            for field in fields {
                let target = query
                    .program()
                    .component(data, field.field)
                    .ok_or(INVALID)?
                    .data;
                facts.join(&forward(query, field.value, target, state)?);
            }
            Ok(facts)
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
                forward(query, *seed, array.element(), state)
            }
        }
        ExpressionKind::Place(_)
        | ExpressionKind::Move(_)
        | ExpressionKind::Boundary { .. }
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::Index { .. }
        | ExpressionKind::Select { .. } => shape(query, id, state, true),
        _ => Ok(Facts::default()),
    }
}
