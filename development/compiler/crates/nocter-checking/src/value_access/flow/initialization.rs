use super::{Graph, IntegrityError, Operation};
use crate::value_access::{ArgumentAction, ExpressionKind, LocalOrigin, Query as TypedQuery};
use nocter_model::{AccessFlowPointId, AccessLocalId, AccessPlaceId, FieldPath, ValueAccessMode};
mod state;
use state::Storage;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Location {
    pub point: AccessFlowPointId,
    pub action: usize,
}

/// Slot state, not proof that a retained access's outside referent is valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationState {
    Uninitialized,
    Initialized,
    Maybe,
}
impl InitializationState {
    fn join(self, other: Self) -> Self {
        if self == other { self } else { Self::Maybe }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Slot {
    state: InitializationState,
    causes: BTreeSet<Location>,
}
type State = BTreeMap<AccessLocalId, Storage>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationRequirement {
    Value,
    Container,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationViolation {
    pub requirement: InitializationRequirement,
    pub location: Location,
    pub local: AccessLocalId,
    pub fields: FieldPath,
    pub state: InitializationState,
    pub causes: Box<[Location]>,
}

/// Initialization condition at a potential owned-slot cleanup. Not an executable cleanup schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupFact {
    pub location: Location,
    pub local: AccessLocalId,
    pub fields: FieldPath,
    pub state: InitializationState,
}

#[derive(Clone, Debug)]
pub struct Initialization {
    entries: BTreeMap<AccessFlowPointId, State>,
    violations: Box<[InitializationViolation]>,
    cleanup: Box<[CleanupFact]>,
}
impl Initialization {
    #[must_use]
    pub const fn violations(&self) -> &[InitializationViolation] {
        &self.violations
    }
    #[must_use]
    pub const fn cleanup(&self) -> &[CleanupFact] {
        &self.cleanup
    }
    #[must_use]
    pub fn reachable(&self, point: AccessFlowPointId) -> bool {
        self.entries.contains_key(&point)
    }
    #[must_use]
    pub fn at_entry(
        &self,
        point: AccessFlowPointId,
        local: AccessLocalId,
    ) -> Option<InitializationState> {
        self.entries
            .get(&point)?
            .get(&local)
            .map(|slot| slot.complete(&FieldPath::default()).state)
    }
}

pub(super) fn analyze(typed: &TypedQuery, graph: &Graph) -> Result<Initialization, IntegrityError> {
    let entry: State = typed
        .body()
        .locals()
        .iter()
        .map(|(id, local)| {
            (
                id,
                Storage::new(if matches!(local.origin, LocalOrigin::Parameter(_)) {
                    InitializationState::Initialized
                } else {
                    InitializationState::Uninitialized
                }),
            )
        })
        .collect();
    let mut entries = BTreeMap::from([(graph.entry(), entry)]);
    let mut pending = BTreeSet::from([graph.entry()]);
    while let Some(id) = pending.pop_first() {
        let mut state = entries.get(&id).ok_or(INVALID)?.clone();
        transfer(
            typed,
            graph,
            id,
            &mut state,
            &mut Vec::new(),
            &mut Vec::new(),
        )?;
        for successor in graph
            .points()
            .get(id)
            .ok_or(INVALID)?
            .terminator
            .successors()
        {
            let changed = if let Some(current) = entries.get_mut(&successor) {
                merge(current, &state)?
            } else {
                entries.insert(successor, state.clone());
                true
            };
            if changed {
                pending.insert(successor);
            }
        }
    }
    // Report only after convergence. First-iteration success cannot certify a loop backedge.
    let mut violations = Vec::new();
    let mut cleanup = Vec::new();
    for (&point, state) in &entries {
        transfer(
            typed,
            graph,
            point,
            &mut state.clone(),
            &mut violations,
            &mut cleanup,
        )?;
    }
    Ok(Initialization {
        entries,
        violations: violations.into_boxed_slice(),
        cleanup: cleanup.into_boxed_slice(),
    })
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
fn merge(current: &mut State, incoming: &State) -> Result<bool, IntegrityError> {
    let mut changed = false;
    for (&local, from) in incoming {
        let to = current.get_mut(&local).ok_or(INVALID)?;
        changed |= to.merge(from);
    }
    Ok(changed)
}
fn selected(
    typed: &TypedQuery,
    id: AccessPlaceId,
) -> Result<&crate::value_access::Place, IntegrityError> {
    typed.body().places().get(id).ok_or(INVALID)
}
fn require(
    state: &State,
    local: AccessLocalId,
    fields: &FieldPath,
    requirement: InitializationRequirement,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let storage = state.get(&local).ok_or(INVALID)?;
    let slot = if requirement == InitializationRequirement::Value {
        storage.complete(fields)
    } else {
        storage.present(fields)
    };
    if slot.state != InitializationState::Initialized {
        violations.push(InitializationViolation {
            requirement,
            location,
            local,
            fields: fields.clone(),
            state: slot.state,
            causes: slot.causes.into_iter().collect(),
        });
    }
    Ok(())
}
fn require_place(
    typed: &TypedQuery,
    state: &State,
    place: AccessPlaceId,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let p = selected(typed, place)?;
    require(
        state,
        p.local,
        &p.fields,
        InitializationRequirement::Value,
        at,
        violations,
    )
}
fn remove(
    state: &mut State,
    local: AccessLocalId,
    fields: &FieldPath,
    at: Location,
) -> Result<(), IntegrityError> {
    state
        .get_mut(&local)
        .ok_or(INVALID)?
        .set(fields, InitializationState::Uninitialized, Some(at));
    Ok(())
}
fn remove_place(
    typed: &TypedQuery,
    state: &mut State,
    place: AccessPlaceId,
    at: Location,
) -> Result<(), IntegrityError> {
    let p = selected(typed, place)?;
    remove(state, p.local, &p.fields, at)
}
fn initialize(
    state: &mut State,
    local: AccessLocalId,
    fields: &FieldPath,
) -> Result<(), IntegrityError> {
    state
        .get_mut(&local)
        .ok_or(INVALID)?
        .set(fields, InitializationState::Initialized, None);
    Ok(())
}
fn cleanup_fact(
    typed: &TypedQuery,
    state: &State,
    local: AccessLocalId,
    fields: &FieldPath,
    location: Location,
    cleanup: &mut Vec<CleanupFact>,
) -> Result<(), IntegrityError> {
    if typed
        .body()
        .locals()
        .get(local)
        .ok_or(INVALID)?
        .access()
        .mode()
        == ValueAccessMode::Own
    {
        cleanup.push(CleanupFact {
            location,
            local,
            fields: fields.clone(),
            state: state.get(&local).ok_or(INVALID)?.present(fields).state,
        });
    }
    Ok(())
}
fn arguments(
    typed: &TypedQuery,
    call: nocter_model::AccessExpressionId,
) -> Result<&[crate::value_access::Argument], IntegrityError> {
    let ExpressionKind::Call { arguments, .. } =
        &typed.body().expressions().get(call).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    Ok(arguments)
}
fn transfer(
    typed: &TypedQuery,
    graph: &Graph,
    point: AccessFlowPointId,
    state: &mut State,
    violations: &mut Vec<InitializationViolation>,
    cleanup: &mut Vec<CleanupFact>,
) -> Result<(), IntegrityError> {
    for (action, event) in graph
        .points()
        .get(point)
        .ok_or(INVALID)?
        .actions
        .iter()
        .enumerate()
    {
        let location = Location { point, action };
        match &event.operation {
            Operation::Evaluate(id) => evaluate(typed, *id, state, location, violations)?,
            Operation::PrepareArgument { call, argument } => {
                match arguments(typed, *call)?
                    .get(*argument)
                    .ok_or(INVALID)?
                    .action
                {
                    ArgumentAction::ReserveRead(place)
                    | ArgumentAction::ReserveUpdate(place)
                    | ArgumentAction::ReserveTransfer(place) => {
                        require_place(typed, state, place, location, violations)?;
                    }
                    ArgumentAction::ReadOwnedValue
                    | ArgumentAction::CopyPayload
                    | ArgumentAction::TakeValue => {}
                    ArgumentAction::Interrupted => return Err(INVALID),
                }
            }
            Operation::CommitCall(call) => {
                // Validate the whole reserved bundle first; consuming one argument cannot make
                // another appear uninitialized inside the atomic transition. Aliasing is separate.
                for argument in arguments(typed, *call)? {
                    if let ArgumentAction::ReserveRead(place)
                    | ArgumentAction::ReserveUpdate(place)
                    | ArgumentAction::ReserveTransfer(place) = argument.action
                    {
                        require_place(typed, state, place, location, violations)?;
                    }
                }
                for argument in arguments(typed, *call)? {
                    if let ArgumentAction::ReserveTransfer(place) = argument.action {
                        remove_place(typed, state, place, location)?;
                    }
                }
            }
            Operation::Bind { local, .. } => initialize(state, *local, &FieldPath::default())?,
            Operation::Assign { target, .. } => {
                assign(typed, state, *target, location, violations, cleanup)?;
            }
            Operation::CompoundAssign { target, .. } => {
                require_place(typed, state, *target, location, violations)?;
            }
            Operation::Drop(local) => {
                require(
                    state,
                    *local,
                    &FieldPath::default(),
                    InitializationRequirement::Container,
                    location,
                    violations,
                )?;
                cleanup_fact(
                    typed,
                    state,
                    *local,
                    &FieldPath::default(),
                    location,
                    cleanup,
                )?;
                remove(state, *local, &FieldPath::default(), location)?;
            }
            Operation::ExitScope { locals, .. } => {
                for &local in locals {
                    cleanup_fact(
                        typed,
                        state,
                        local,
                        &FieldPath::default(),
                        location,
                        cleanup,
                    )?;
                    remove(state, local, &FieldPath::default(), location)?;
                }
            }
            Operation::Forward { .. }
            | Operation::BeginEvaluation(_)
            | Operation::EndEvaluation(_)
            | Operation::BooleanAnswer { .. }
            | Operation::BooleanConstant { .. }
            | Operation::PrepareOperand { .. }
            | Operation::Condition(_)
            | Operation::CancelCall { .. }
            | Operation::Discard(_)
            | Operation::StageReturn(_) => {}
        }
    }
    Ok(())
}

fn evaluate(
    typed: &TypedQuery,
    id: nocter_model::AccessExpressionId,
    state: &mut State,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    match &typed.body().expressions().get(id).ok_or(INVALID)?.kind {
        ExpressionKind::Place(place) => {
            require_place(typed, state, *place, location, violations)?;
        }
        ExpressionKind::Move(place) => {
            require_place(typed, state, *place, location, violations)?;
            remove_place(typed, state, *place, location)?;
        }
        ExpressionKind::Boundary { source, .. } => {
            if let ExpressionKind::Place(place) =
                typed.body().expressions().get(*source).ok_or(INVALID)?.kind
            {
                require_place(typed, state, place, location, violations)?;
            }
        }
        ExpressionKind::Function(_)
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::Select { .. }
        | ExpressionKind::Record { .. }
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. } => {}
        // Structured control and calls have their own ordered graph operations.
        ExpressionKind::Call { .. } | ExpressionKind::If { .. } => return Err(INVALID),
    }
    Ok(())
}

fn assign(
    typed: &TypedQuery,
    state: &mut State,
    target: AccessPlaceId,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
    cleanup: &mut Vec<CleanupFact>,
) -> Result<(), IntegrityError> {
    let place = selected(typed, target)?;
    let mode = typed
        .body()
        .locals()
        .get(place.local)
        .ok_or(INVALID)?
        .access()
        .mode();
    if mode == ValueAccessMode::Update {
        require_place(typed, state, target, location, violations)?;
    } else {
        // Replacing a missing field is allowed; missing containing records are not.
        for len in 0..place.fields.fields().len() {
            require(
                state,
                place.local,
                &FieldPath::new(place.fields.fields()[..len].to_vec()),
                InitializationRequirement::Container,
                location,
                violations,
            )?;
        }
    }
    cleanup_fact(typed, state, place.local, &place.fields, location, cleanup)?;
    initialize(state, place.local, &place.fields)?;
    Ok(())
}
