use super::{Graph, IntegrityError, Operation};
use crate::value_access::{ArgumentAction, ExpressionKind, LocalOrigin, Query as TypedQuery};
use nocter_model::{AccessFlowPointId, AccessLocalId, ValueAccessMode};
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
type State = BTreeMap<AccessLocalId, Slot>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationViolation {
    pub location: Location,
    pub local: AccessLocalId,
    pub state: InitializationState,
    pub causes: Box<[Location]>,
}

/// Initialization condition at a potential owned-slot cleanup. Not an executable cleanup schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupFact {
    pub location: Location,
    pub local: AccessLocalId,
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
        self.entries.get(&point)?.get(&local).map(|slot| slot.state)
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
                Slot {
                    state: if matches!(local.origin, LocalOrigin::Parameter(_)) {
                        InitializationState::Initialized
                    } else {
                        InitializationState::Uninitialized
                    },
                    causes: BTreeSet::new(),
                },
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
        let joined = to.state.join(from.state);
        changed |= joined != to.state;
        to.state = joined;
        let count = to.causes.len();
        to.causes.extend(from.causes.iter().copied());
        changed |= count != to.causes.len();
    }
    Ok(changed)
}
fn require(
    state: &State,
    local: AccessLocalId,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let slot = state.get(&local).ok_or(INVALID)?;
    if slot.state != InitializationState::Initialized {
        violations.push(InitializationViolation {
            location,
            local,
            state: slot.state,
            causes: slot.causes.iter().copied().collect(),
        });
    }
    Ok(())
}
fn remove(
    state: &mut State,
    local: AccessLocalId,
    location: Location,
) -> Result<(), IntegrityError> {
    let slot = state.get_mut(&local).ok_or(INVALID)?;
    *slot = Slot {
        state: InitializationState::Uninitialized,
        causes: BTreeSet::from([location]),
    };
    Ok(())
}
fn initialize(state: &mut State, local: AccessLocalId) -> Result<(), IntegrityError> {
    let slot = state.get_mut(&local).ok_or(INVALID)?;
    *slot = Slot {
        state: InitializationState::Initialized,
        causes: BTreeSet::new(),
    };
    Ok(())
}
fn cleanup_fact(
    typed: &TypedQuery,
    state: &State,
    local: AccessLocalId,
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
            state: state.get(&local).ok_or(INVALID)?.state,
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
                    ArgumentAction::ReserveRead(local)
                    | ArgumentAction::ReserveUpdate(local)
                    | ArgumentAction::ReserveTransfer(local) => {
                        require(state, local, location, violations)?;
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
                    if let ArgumentAction::ReserveRead(local)
                    | ArgumentAction::ReserveUpdate(local)
                    | ArgumentAction::ReserveTransfer(local) = argument.action
                    {
                        require(state, local, location, violations)?;
                    }
                }
                for argument in arguments(typed, *call)? {
                    if let ArgumentAction::ReserveTransfer(local) = argument.action {
                        remove(state, local, location)?;
                    }
                }
            }
            Operation::Bind { local, .. } => initialize(state, *local)?,
            Operation::Assign { target, .. } => {
                // U updates the referent, so its holder must already be initialized. X can restore.
                if typed
                    .body()
                    .locals()
                    .get(*target)
                    .ok_or(INVALID)?
                    .access()
                    .mode()
                    == ValueAccessMode::Update
                {
                    require(state, *target, location, violations)?;
                }
                cleanup_fact(typed, state, *target, location, cleanup)?;
                initialize(state, *target)?;
            }
            Operation::CompoundAssign { target, .. } => {
                require(state, *target, location, violations)?;
            }
            Operation::Drop(local) => {
                require(state, *local, location, violations)?;
                cleanup_fact(typed, state, *local, location, cleanup)?;
                remove(state, *local, location)?;
            }
            Operation::ExitScope { locals, .. } => {
                for &local in locals {
                    cleanup_fact(typed, state, local, location, cleanup)?;
                    remove(state, local, location)?;
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
        ExpressionKind::Place(local) => {
            require(state, *local, location, violations)?;
        }
        ExpressionKind::Move(local) => {
            require(state, *local, location, violations)?;
            remove(state, *local, location)?;
        }
        ExpressionKind::Boundary { source, .. } => {
            if let ExpressionKind::Place(local) =
                typed.body().expressions().get(*source).ok_or(INVALID)?.kind
            {
                require(state, local, location, violations)?;
            }
        }
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. } => {}
        // Structured control and calls have their own ordered graph operations.
        ExpressionKind::Call { .. } | ExpressionKind::If { .. } => return Err(INVALID),
    }
    Ok(())
}
