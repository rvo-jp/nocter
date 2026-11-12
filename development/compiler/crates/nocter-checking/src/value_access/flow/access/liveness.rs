use super::super::{Graph, Initialization, IntegrityError, Location, Operation};
use crate::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Query as TypedQuery, Referent,
};
use nocter_model::{AccessExpressionId, AccessFlowPointId, AccessLocalId, ValueAccessMode as Mode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Carrier {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Argument {
        call: AccessExpressionId,
        index: usize,
    },
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Demand {
    pub uses: BTreeSet<Location>,
    pub updates: BTreeSet<Location>,
}
pub(super) type Live = BTreeMap<Carrier, Demand>;
#[derive(Clone, Debug, Default)]
pub struct Liveness {
    entries: BTreeMap<AccessFlowPointId, Live>,
    before: BTreeMap<Location, Live>,
    after: BTreeMap<Location, Live>,
}
impl Liveness {
    #[must_use]
    pub fn required_after(&self, location: Location, carrier: Carrier) -> bool {
        self.after(location)
            .is_some_and(|live| live.contains_key(&carrier))
    }
    #[must_use]
    pub fn required_use(&self, location: Location, carrier: Carrier) -> Option<Location> {
        self.before(location)?.get(&carrier)?.uses.first().copied()
    }
    #[must_use]
    pub fn required_update(&self, location: Location, carrier: Carrier) -> Option<Location> {
        self.before(location)?
            .get(&carrier)?
            .updates
            .first()
            .copied()
    }
    pub(super) fn entry(&self, point: AccessFlowPointId) -> Option<&Live> {
        self.entries.get(&point)
    }
    pub(super) fn before(&self, location: Location) -> Option<&Live> {
        self.before.get(&location)
    }
    pub(super) fn after(&self, location: Location) -> Option<&Live> {
        self.after.get(&location)
    }
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
fn union(to: &mut Live, from: &Live) {
    for (&carrier, uses) in from {
        let demand = to.entry(carrier).or_default();
        demand.uses.extend(&uses.uses);
        demand.updates.extend(&uses.updates);
    }
}
fn updating(live: &Live, carrier: Carrier) -> bool {
    live.get(&carrier)
        .is_some_and(|demand| !demand.updates.is_empty())
}
fn argument(
    typed: &TypedQuery,
    call: AccessExpressionId,
    index: usize,
) -> Result<&crate::value_access::Argument, IntegrityError> {
    let ExpressionKind::Call { arguments, .. } =
        &typed.body().expressions().get(call).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    arguments.get(index).ok_or(INVALID)
}
fn argument_updates(argument: &crate::value_access::Argument) -> bool {
    argument.operation == nocter_model::ValueInputOperation::Edit
}
#[derive(Clone, Copy)]
enum Need {
    Release,
    Read,
    Update,
}
impl From<bool> for Need {
    fn from(update: bool) -> Self {
        if update { Self::Update } else { Self::Read }
    }
}
type Uses = Vec<(Carrier, Need)>;
fn prepared_use(
    typed: &TypedQuery,
    call: AccessExpressionId,
    index: usize,
) -> Result<(Carrier, bool), IntegrityError> {
    let arg = argument(typed, call, index)?;
    let carrier = match arg.action {
        ArgumentAction::ReserveRead(place)
        | ArgumentAction::ReserveUpdate(place)
        | ArgumentAction::ReserveTransfer { place, .. }
        | ArgumentAction::CopyPlace(place) => referent_carrier(typed, place)?,
        _ => Carrier::Expression(arg.value),
    };
    Ok((carrier, argument_updates(arg)))
}

fn replaced_local(
    typed: &TypedQuery,
    target: Referent,
) -> Result<Option<AccessLocalId>, IntegrityError> {
    let Some(place) = target.place() else {
        return Ok(None);
    };
    let place = typed.body().places().get(place).ok_or(INVALID)?;
    let local = typed.body().locals().get(place.local).ok_or(INVALID)?;
    Ok((place.fields.is_root() && local.access().mode() == Mode::Own).then_some(place.local))
}

fn index_failure_uses(
    typed: &TypedQuery,
    id: AccessExpressionId,
) -> Result<[(Carrier, bool); 2], IntegrityError> {
    let ExpressionKind::Index {
        base,
        index,
        bounds: nocter_model::IndexBoundsCheck::ProvenTrap,
        ..
    } = typed.body().expressions().get(id).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    Ok([
        (Carrier::Expression(base), false),
        (Carrier::Expression(index), false),
    ])
}

fn plan(
    typed: &TypedQuery,
    operation: &Operation,
    after: &Live,
) -> Result<(Uses, Vec<Carrier>), IntegrityError> {
    let mut uses: Vec<(Carrier, bool)> = Vec::new();
    let mut definitions = Vec::new();
    let mut releases = Vec::new();
    match operation {
        Operation::IndexFailure(id) => {
            uses.extend(index_failure_uses(typed, *id)?);
        }
        Operation::BeginEvaluation(_) | Operation::EndEvaluation(_) => {}
        Operation::Evaluate(id) => {
            definitions.push(Carrier::Expression(*id));
            uses.extend(expression_uses(typed, *id, after)?);
        }
        Operation::PrepareOperand { value, .. }
        | Operation::Condition(value)
        | Operation::Discard(value)
        | Operation::StageReturn(value) => uses.push((Carrier::Expression(*value), false)),
        Operation::Forward { source, result } => {
            uses.push((
                Carrier::Expression(*source),
                updating(after, Carrier::Expression(*result)),
            ));
            definitions.push(Carrier::Expression(*result));
        }
        Operation::BooleanAnswer { source, result } => {
            uses.push((Carrier::Expression(*source), false));
            definitions.push(Carrier::Expression(*result));
        }
        Operation::BooleanConstant { result, .. } => definitions.push(Carrier::Expression(*result)),
        Operation::Bind { local, value } => {
            uses.push((
                Carrier::Expression(*value),
                updating(after, Carrier::Local(*local)),
            ));
            definitions.push(Carrier::Local(*local));
        }
        Operation::Assign { target, value } => {
            uses.push((Carrier::Expression(*value), false));
            if let Some(local) = replaced_local(typed, *target)? {
                definitions.push(Carrier::Local(local));
            } else {
                uses.push((referent_carrier(typed, *target)?, true));
            }
        }
        Operation::CompoundAssign { target, value, .. } => {
            uses.push((Carrier::Expression(*value), false));
            uses.push((referent_carrier(typed, *target)?, true));
        }
        Operation::Drop(local) => {
            uses.push((Carrier::Local(*local), true));
            definitions.push(Carrier::Local(*local));
        }
        Operation::ExitScope { locals, .. } => {
            definitions.extend(locals.iter().copied().map(Carrier::Local));
        }
        Operation::PrepareArgument {
            call,
            argument: index,
        } => {
            uses.push(prepared_use(typed, *call, *index)?);
            definitions.push(Carrier::Argument {
                call: *call,
                index: *index,
            });
        }
        Operation::CommitCall(call) => {
            let ExpressionKind::Call { arguments, .. } =
                &typed.body().expressions().get(*call).ok_or(INVALID)?.kind
            else {
                return Err(INVALID);
            };
            for (index, arg) in arguments.iter().enumerate() {
                let carrier = Carrier::Argument { call: *call, index };
                uses.push((carrier, argument_updates(arg)));
                definitions.push(carrier);
            }
            definitions.push(Carrier::Expression(*call));
        }
        Operation::CancelCall { call, prepared } => {
            for index in 0..*prepared {
                let carrier = Carrier::Argument { call: *call, index };
                releases.push(carrier);
                definitions.push(carrier);
            }
        }
    }
    Ok((
        uses.into_iter()
            .map(|(carrier, update)| (carrier, Need::from(update)))
            .chain(releases.into_iter().map(|carrier| (carrier, Need::Release)))
            .collect(),
        definitions,
    ))
}
pub(super) fn analyze(
    typed: &TypedQuery,
    graph: &Graph,
    initialization: &Initialization,
) -> Result<Liveness, IntegrityError> {
    let mut result = Liveness::default();
    let points = graph
        .points()
        .iter()
        .filter(|(id, _)| initialization.reachable(*id))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for &(point, body) in points.iter().rev() {
            let mut live = Live::new();
            for next in body.terminator.successors() {
                if let Some(entry) = result.entries.get(&next) {
                    union(&mut live, entry);
                }
            }
            for (action, event) in body.actions.iter().enumerate().rev() {
                let location = Location { point, action };
                result.after.insert(location, live.clone());
                let (uses, definitions) = plan(typed, &event.operation, &live)?;
                for definition in definitions {
                    live.remove(&definition);
                }
                for (used, update) in uses {
                    let demand = live.entry(used).or_default();
                    if !matches!(update, Need::Release) {
                        demand.uses.insert(location);
                    }
                    if matches!(update, Need::Update) {
                        demand.updates.insert(location);
                    }
                }
                result.before.insert(location, live.clone());
            }
            changed |= result.entries.get(&point) != Some(&live);
            result.entries.insert(point, live);
        }
        if !changed {
            return Ok(result);
        }
    }
}

fn expression_uses(
    typed: &TypedQuery,
    id: AccessExpressionId,
    after: &Live,
) -> Result<Vec<(Carrier, bool)>, IntegrityError> {
    let update = updating(after, Carrier::Expression(id));
    let mut uses = Vec::new();
    match typed.body().expressions().get(id).ok_or(INVALID)?.kind {
        ExpressionKind::Place(place) => uses.push((Carrier::Local(root(typed, place)?), update)),
        ExpressionKind::Move(place) => {
            uses.push((referent_carrier(typed, place)?, true));
        }
        ExpressionKind::Boundary { source, action } => {
            let carrier = match typed.body().expressions().get(source).ok_or(INVALID)?.kind {
                ExpressionKind::Place(place) => Carrier::Local(root(typed, place)?),
                _ => Carrier::Expression(source),
            };
            uses.push((carrier, action == BoundaryAction::DelegateUpdate));
        }
        ExpressionKind::Index {
            base,
            index,
            action,
            ..
        } => {
            uses.push((
                Carrier::Expression(base),
                update && action == crate::value_access::IndexAction::Access,
            ));
            uses.push((Carrier::Expression(index), false));
        }
        ExpressionKind::Select { source, action, .. } => {
            uses.push((
                Carrier::Expression(source),
                update && action == crate::value_access::SelectionAction::Access,
            ));
        }
        ExpressionKind::RequirePrimary { source }
        | ExpressionKind::Repeat { seed: source }
        | ExpressionKind::AdaptFunction { source }
        | ExpressionKind::Closure {
            capture: source, ..
        } => {
            uses.push((Carrier::Expression(source), false));
        }
        ExpressionKind::Aggregate { ref fields } => {
            uses.extend(
                fields
                    .iter()
                    .map(|field| (Carrier::Expression(field.value), false)),
            );
        }
        ExpressionKind::Function(_)
        | ExpressionKind::Constant(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. } => {}
        ExpressionKind::Call { .. } | ExpressionKind::If { .. } => return Err(INVALID),
    }
    Ok(uses)
}

fn referent_carrier(typed: &TypedQuery, referent: Referent) -> Result<Carrier, IntegrityError> {
    match referent {
        Referent::Place(place) => Ok(Carrier::Local(root(typed, place)?)),
        Referent::Evaluated(id) if typed.body().referent_data(referent).is_some() => {
            Ok(Carrier::Expression(id))
        }
        Referent::Evaluated(_) => Err(INVALID),
    }
}
fn root(
    typed: &TypedQuery,
    place: nocter_model::AccessPlaceId,
) -> Result<AccessLocalId, IntegrityError> {
    Ok(typed.body().places().get(place).ok_or(INVALID)?.local)
}
