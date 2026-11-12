use super::{Graph, IntegrityError, Operation};
use crate::value_access::{
    AccessPath, ArgumentAction, ExpressionKind, Query as TypedQuery, Referent, paths,
};
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
    pub fields: AccessPath,
    pub state: InitializationState,
    pub causes: Box<[Location]>,
}

#[derive(Clone, Debug)]
pub struct Initialization {
    entries: BTreeMap<AccessFlowPointId, State>,
    violations: Box<[InitializationViolation]>,
}
impl Initialization {
    #[must_use]
    pub const fn violations(&self) -> &[InitializationViolation] {
        &self.violations
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

pub(super) fn analyze(
    typed: &TypedQuery,
    graph: &Graph,
    numeric: &super::numeric::Analysis,
) -> Result<Initialization, IntegrityError> {
    let entry: State = typed
        .body()
        .locals()
        .iter()
        .map(|(id, local)| {
            (
                id,
                Storage::new(if local.origin.initialized_on_entry() {
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
        transfer(typed, graph, numeric, id, &mut state, &mut Vec::new())?;
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
    for (&point, state) in &entries {
        transfer(
            typed,
            graph,
            numeric,
            point,
            &mut state.clone(),
            &mut violations,
        )?;
    }
    Ok(Initialization {
        entries,
        violations: violations.into_boxed_slice(),
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
    require_path(
        state,
        local,
        paths::known(fields),
        requirement,
        location,
        violations,
    )
}
fn require_referent(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    referent: Referent,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    require_selection(
        typed,
        facts,
        state,
        referent,
        InitializationRequirement::Value,
        at,
        violations,
    )
}
fn require_selection(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    referent: Referent,
    requirement: InitializationRequirement,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    for (local, fields) in
        super::numeric::initialization_paths(typed, facts, referent).ok_or(INVALID)?
    {
        require_path(state, local, fields, requirement, at, violations)?;
    }
    // Control receipts validated their selected source at Forward, before scope cleanup.
    // Access analysis preserves the actual referent; never re-read an expired alias slot.
    Ok(())
}
fn require_path(
    state: &State,
    local: AccessLocalId,
    fields: AccessPath,
    requirement: InitializationRequirement,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let slot = state
        .get(&local)
        .ok_or(INVALID)?
        .selection(&fields, requirement);
    if slot.state != InitializationState::Initialized {
        violations.push(InitializationViolation {
            requirement,
            location: at,
            local,
            fields,
            state: slot.state,
            causes: slot.causes.into_iter().collect(),
        });
    }
    Ok(())
}
fn require_expression(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    id: nocter_model::AccessExpressionId,
    requirement: InitializationRequirement,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    if let Some(referent) = typed
        .body()
        .expressions()
        .get(id)
        .ok_or(INVALID)?
        .referent(id)
    {
        require_selection(typed, facts, state, referent, requirement, at, violations)?;
    }
    Ok(())
}
fn require_formation(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    id: nocter_model::AccessExpressionId,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    if let Some(referent) = typed
        .body()
        .expressions()
        .get(id)
        .ok_or(INVALID)?
        .referent(id)
    {
        for (local, path) in
            super::numeric::initialization_paths(typed, facts, referent).ok_or(INVALID)?
        {
            // A child slot can be missing while its containing array/record remains present.
            let parent = AccessPath::new(
                path.fields()
                    .iter()
                    .take(path.fields().len().saturating_sub(1))
                    .copied(),
            );
            require_path(
                state,
                local,
                parent,
                InitializationRequirement::Container,
                at,
                violations,
            )?;
        }
    }
    Ok(())
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
fn remove_referent(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &mut State,
    referent: Referent,
    at: Location,
) -> Result<(), IntegrityError> {
    let (local, path) = super::numeric::path(typed, facts, referent).ok_or(INVALID)?;
    state.get_mut(&local).ok_or(INVALID)?.remove_path(&path, at);
    Ok(())
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
fn require_array_container(
    state: &State,
    array: &super::resources::Place,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let super::resources::Holder::Local(local) = array.holder() else {
        return Err(INVALID);
    };
    require(
        state,
        local,
        &array.fields().concrete().ok_or(INVALID)?,
        InitializationRequirement::Container,
        location,
        violations,
    )
}

fn require_committed_argument(
    typed: &TypedQuery,
    numeric: &super::numeric::Analysis,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    argument: &ArgumentAction,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    if let ArgumentAction::ReserveTransfer {
        place: Referent::Evaluated(selection),
        ..
    } = argument
        && let Some(frontier) = numeric.prefix(location, *selection)
    {
        let array = frontier.array(typed).ok_or(INVALID)?;
        require_array_container(state, &array, location, violations)
    } else if let ArgumentAction::ReserveRead(place)
    | ArgumentAction::ReserveUpdate(place)
    | ArgumentAction::ReserveTransfer { place, .. } = *argument
    {
        require_referent(typed, facts, state, place, location, violations)
    } else {
        Ok(())
    }
}

fn require_prepared_argument(
    typed: &TypedQuery,
    numeric: &super::numeric::Analysis,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    argument: &ArgumentAction,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    match *argument {
        ArgumentAction::ReserveRead(place)
        | ArgumentAction::ReserveUpdate(place)
        | ArgumentAction::ReserveTransfer { place, .. }
        | ArgumentAction::CopyPlace(place) => {
            if let Some(array) = numeric.available(location) {
                require_array_container(state, array, location, violations)
            } else {
                require_referent(typed, facts, state, place, location, violations)
            }
        }
        ArgumentAction::AccessOwnedValue | ArgumentAction::TakeValue => Ok(()),
        ArgumentAction::PendingValue(_)
        | ArgumentAction::Unproduced
        | ArgumentAction::Interrupted => Err(INVALID),
    }
}

fn require_forwarded(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &State,
    source: nocter_model::AccessExpressionId,
    result: nocter_model::AccessExpressionId,
    at: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    if typed
        .body()
        .expressions()
        .get(result)
        .ok_or(INVALID)?
        .referent(result)
        .is_some()
    {
        require_expression(
            typed,
            facts,
            state,
            source,
            InitializationRequirement::Value,
            at,
            violations,
        )?;
    }
    Ok(())
}

fn transfer(
    typed: &TypedQuery,
    graph: &Graph,
    numeric: &super::numeric::Analysis,
    point: AccessFlowPointId,
    state: &mut State,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    let mut cursor = numeric.cursor(typed, point);
    for (action, event) in graph
        .points()
        .get(point)
        .ok_or(INVALID)?
        .actions
        .iter()
        .enumerate()
    {
        let location = Location { point, action };
        let facts = cursor.next(&event.operation)?;
        let facts = facts.as_ref();
        match &event.operation {
            Operation::Evaluate(id) => {
                evaluate(typed, numeric, facts, *id, state, location, violations)?;
            }
            Operation::PrepareArgument { call, argument } => {
                require_prepared_argument(
                    typed,
                    numeric,
                    facts,
                    state,
                    &arguments(typed, *call)?
                        .get(*argument)
                        .ok_or(INVALID)?
                        .action,
                    location,
                    violations,
                )?;
            }
            Operation::CommitCall(call) => {
                // Validate the whole reserved bundle first; consuming one argument cannot make
                // another appear uninitialized inside the atomic transition. Aliasing is separate.
                for argument in arguments(typed, *call)? {
                    require_committed_argument(
                        typed,
                        numeric,
                        facts,
                        state,
                        &argument.action,
                        location,
                        violations,
                    )?;
                }
                for argument in arguments(typed, *call)? {
                    if let ArgumentAction::ReserveTransfer { place, .. } = argument.action {
                        remove_referent(typed, facts, state, place, location)?;
                    }
                }
            }
            Operation::Bind { local, .. } => initialize(state, *local, &FieldPath::default())?,
            Operation::Assign { target, .. } => {
                assign(typed, facts, state, *target, location, violations)?;
            }
            Operation::CompoundAssign { target, .. } => {
                require_referent(typed, facts, state, *target, location, violations)?;
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
                remove(state, *local, &FieldPath::default(), location)?;
            }
            Operation::ExitScope { locals, .. } => {
                for &local in locals {
                    remove(state, local, &FieldPath::default(), location)?;
                }
            }
            Operation::PrepareOperand { value, .. }
            | Operation::Condition(value)
            | Operation::Discard(value)
            | Operation::StageReturn(value)
            | Operation::BooleanAnswer { source: value, .. } => {
                require_expression(
                    typed,
                    facts,
                    state,
                    *value,
                    InitializationRequirement::Value,
                    location,
                    violations,
                )?;
            }
            Operation::Forward { source, result, .. } => {
                require_forwarded(typed, facts, state, *source, *result, location, violations)?;
            }
            Operation::IndexFailure(_)
            | Operation::BeginEvaluation(_)
            | Operation::EndEvaluation(_)
            | Operation::BooleanConstant { .. }
            | Operation::CancelCall { .. } => {}
        }
    }
    Ok(())
}

fn evaluate(
    typed: &TypedQuery,
    numeric: &super::numeric::Analysis,
    facts: Option<&super::numeric::NumericFacts>,
    id: nocter_model::AccessExpressionId,
    state: &mut State,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    match &typed.body().expressions().get(id).ok_or(INVALID)?.kind {
        ExpressionKind::Move(place) => {
            if let Referent::Evaluated(selection) = *place
                && let Some(frontier) = numeric.prefix(location, selection)
            {
                let array = frontier.array(typed).ok_or(INVALID)?;
                require_array_container(state, &array, location, violations)?;
            } else {
                require_referent(typed, facts, state, *place, location, violations)?;
            }
            remove_referent(typed, facts, state, *place, location)?;
        }
        // Pending supply has no availability semantics. Calls/control use their own graph events.
        ExpressionKind::Boundary {
            action:
                crate::value_access::BoundaryAction::PendingValue
                | crate::value_access::BoundaryAction::Unproduced,
            ..
        }
        | ExpressionKind::Call { .. }
        | ExpressionKind::Scoped { .. }
        | ExpressionKind::Block { .. }
        | ExpressionKind::If { .. } => return Err(INVALID),
        ExpressionKind::Place(_) | ExpressionKind::Index { .. } | ExpressionKind::Select { .. } => {
            require_formation(typed, facts, state, id, location, violations)?;
        }
        ExpressionKind::GuardView { source, .. } | ExpressionKind::PrimaryCondition { source } => {
            require_referent(typed, facts, state, *source, location, violations)?;
        }
        ExpressionKind::RequirePrimary { source } | ExpressionKind::Boundary { source, .. } => {
            require_expression(
                typed,
                facts,
                state,
                *source,
                InitializationRequirement::Value,
                location,
                violations,
            )?;
        }
        ExpressionKind::Function(_)
        | ExpressionKind::Closure { .. }
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::Aggregate { .. }
        | ExpressionKind::Repeat { .. }
        | ExpressionKind::Constant(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. } => {}
    }
    Ok(())
}

fn restore_conditional_point(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &mut State,
    target: Referent,
) -> Result<bool, IntegrityError> {
    let Some(point) = facts.and_then(|facts| facts.conditional_restoration(typed, target)) else {
        return Ok(false);
    };
    let parent = point.array.fields().concrete().ok_or(INVALID)?;
    let storage = state.get_mut(&point.local).ok_or(INVALID)?;
    if storage.present(&parent).state != InitializationState::Initialized {
        return Ok(false);
    }
    Ok(storage.restore_conditional_point(&parent, &point.retirements))
}

fn restore_point(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &mut State,
    target: Referent,
) -> Result<bool, IntegrityError> {
    if restore_conditional_point(typed, facts, state, target)? {
        return Ok(true);
    }
    let Referent::Evaluated(selection) = target else {
        return Ok(false);
    };
    let Some(point) = facts.and_then(|facts| facts.restoration(typed, selection)) else {
        return Ok(false);
    };
    let array = point.source.array(typed).ok_or(INVALID)?;
    let parent = array.fields().concrete().ok_or(INVALID)?;
    let (local, hole) =
        super::numeric::path(typed, None, Referent::Evaluated(point.source.selection()))
            .ok_or(INVALID)?;
    let storage = state.get_mut(&local).ok_or(INVALID)?;
    if storage.present(&parent).state != InitializationState::Initialized {
        return Ok(false);
    }
    Ok(storage.restore_point(&hole, point.retirements))
}

fn assign(
    typed: &TypedQuery,
    facts: Option<&super::numeric::NumericFacts>,
    state: &mut State,
    target: Referent,
    location: Location,
    violations: &mut Vec<InitializationViolation>,
) -> Result<(), IntegrityError> {
    if restore_point(typed, facts, state, target)? {
        return Ok(());
    }
    let place = if let Some(place) = target.place() {
        selected(typed, place)?.clone()
    } else {
        let Some((local, fields)) = super::numeric::path(typed, facts, target) else {
            // A retained alternative place can replace only an already initialized referent.
            // It cannot initialize all candidate roots or erase a hole on an unchosen branch.
            return require_referent(typed, facts, state, target, location, violations);
        };
        let mode = typed
            .body()
            .locals()
            .get(local)
            .ok_or(INVALID)?
            .access()
            .mode();
        let Some(fields) = paths::concrete(&fields).filter(|_| mode == ValueAccessMode::Own) else {
            // An unproved selection cannot erase a hole. Scoped replacement requires a value.
            return require_referent(typed, facts, state, target, location, violations);
        };
        crate::value_access::Place {
            local,
            fields,
            data: typed.body().referent_data(target).ok_or(INVALID)?,
        }
    };
    let mode = typed
        .body()
        .locals()
        .get(place.local)
        .ok_or(INVALID)?
        .access()
        .mode();
    if mode == ValueAccessMode::Update {
        require_referent(typed, facts, state, target, location, violations)?;
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
    if state
        .get(&place.local)
        .ok_or(INVALID)?
        .uncertain_replacement(&place.fields)
    {
        // Until conditional retained-hole restoration is realized, a possibly absent selected
        // value cannot acquire a guessed old-payload cleanup branch.
        require(
            state,
            place.local,
            &place.fields,
            InitializationRequirement::Value,
            location,
            violations,
        )?;
    }
    initialize(state, place.local, &place.fields)?;
    Ok(())
}
