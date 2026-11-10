//! Finite logical ownership and sparse partial-payload cleanup for the closed data contracts.
//! These transitions retain obligations; they do not choose physical storage or an ABI.
use super::{IntegrityError, Location};
use crate::value_access::Query as TypedQuery;
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_model::{AccessExpressionId, AccessLocalId, BuiltinType, FieldPath, TypeId};
use std::collections::{BTreeMap, BTreeSet};

mod assembly;
mod capture;
mod function_view;
mod payload;
mod storage;
#[cfg(test)]
mod tests;
use payload::OwnedValue;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Holder {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Prepared {
        call: AccessExpressionId,
        index: usize,
    },
    Temporary {
        call: AccessExpressionId,
        index: usize,
    },
    Return,
}
impl Holder {
    #[must_use]
    pub fn place(self) -> Place {
        Place {
            holder: self,
            fields: FieldPath::default(),
        }
    }
}

/// A logical selection within one resource holder, not a physical address or an access grant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Place {
    holder: Holder,
    fields: FieldPath,
}
impl Place {
    #[must_use]
    pub const fn holder(&self) -> Holder {
        self.holder
    }
    #[must_use]
    pub const fn fields(&self) -> &FieldPath {
        &self.fields
    }
    #[must_use]
    pub fn project(&self, field: nocter_model::ValueField) -> Self {
        Self {
            holder: self.holder,
            fields: self.fields.project(field),
        }
    }
    #[must_use]
    pub fn project_path(&self, suffix: &FieldPath) -> Self {
        Self {
            holder: self.holder,
            fields: self.fields.concat(suffix),
        }
    }
    /// Relation for declaration-validated selections; not an availability or permission proof.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.holder == other.holder && self.fields.overlaps(&other.fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Disposal {
    OwnedCapture,
    /// The supplied environment's destructor, retained symbolically until witness specialization.
    FunctionEnvironment,
    Trivial,
    OwnedError,
    Aggregate,
}
/// A schema-selected cleanup obligation, concrete or tied to a formal environment witness.
/// Independence is never inferred from Copy, ownership or the absence of known captures.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Payload {
    data: TypeId,
    disposal: Disposal,
}
impl Payload {
    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }
    #[must_use]
    pub const fn disposal(self) -> Disposal {
        self.disposal
    }
    /// The admitted builtin leaves and their closed aggregate compositions have empty D/S here.
    #[must_use]
    pub const fn has_external_dependencies(self) -> bool {
        false
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    Capture {
        source: Place,
        destination: Holder,
        environment: Payload,
        payload: Payload,
    },
    /// Reclassify one selected named witness without constructing or duplicating its value.
    AdaptFunction {
        source: Place,
        destination: Holder,
        from: Payload,
        payload: Payload,
    },
    /// Transfer complete component obligations into one record; no second value is constructed.
    Assemble {
        destination: Holder,
        payload: Payload,
        fields: Box<[(nocter_model::ValueField, Place)]>,
    },
    Construct {
        destination: Holder,
        payload: Payload,
    },
    /// Receive the callee's existing result obligation, without acquiring another resource.
    Receive {
        call: AccessExpressionId,
        destination: Holder,
        payload: Payload,
    },
    Move {
        source: Place,
        destination: Place,
        payload: Payload,
    },
    Dispose {
        source: Place,
        payload: Payload,
    },
    /// End a partial record's containing slot after cleaning its surviving regions. This does
    /// not destroy its payload again or prescribe physical allocation/deallocation.
    ReleaseContainer {
        source: Place,
        payload: Payload,
    },
    Invoke {
        source: Place,
        call: AccessExpressionId,
        index: usize,
        payload: Payload,
    },
    /// Replacement through a caller-owned U input; the old payload is initialized by contract.
    ReplaceInput {
        local: AccessLocalId,
        fields: FieldPath,
        source: Place,
        payload: Payload,
    },
}
impl Step {
    /// Project payload obligations without selecting another operation or changing place identity.
    /// # Errors
    /// Returns the supplied projection's error without publishing a partly transformed step.
    pub fn try_map_payloads<E>(
        &self,
        mut map: impl FnMut(Payload) -> std::result::Result<Payload, E>,
    ) -> std::result::Result<Self, E> {
        let mut step = self.clone();
        match &mut step {
            Self::Capture {
                environment,
                payload,
                ..
            } => {
                *environment = map(*environment)?;
                *payload = map(*payload)?;
            }
            Self::AdaptFunction { from, payload, .. } => {
                *from = map(*from)?;
                *payload = map(*payload)?;
            }
            Self::Assemble { payload, .. }
            | Self::Construct { payload, .. }
            | Self::Receive { payload, .. }
            | Self::Move { payload, .. }
            | Self::Dispose { payload, .. }
            | Self::ReleaseContainer { payload, .. }
            | Self::Invoke { payload, .. }
            | Self::ReplaceInput { payload, .. } => *payload = map(*payload)?,
        }
        Ok(step)
    }
}

/// Exact owning-container presence and sparse missing selections at one operation boundary.
/// This is a checked guard projection, not an access grant or a physical drop-flag layout.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Condition {
    owned: Box<[(Holder, Box<[FieldPath]>)]>,
}
impl Condition {
    /// Listed holders carry an owning container; unlisted holders carry no ownership.
    /// Scoped input storage is independent. Missing paths stay prefix-free.
    pub fn owned(&self) -> impl Iterator<Item = (Holder, &[FieldPath])> {
        self.owned
            .iter()
            .map(|(holder, missing)| (*holder, missing.as_ref()))
    }
}

/// One ordered sequence with an explicit boundary around the selected source operation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StepPlan {
    steps: Box<[Step]>,
    before: usize,
    operation_state: Condition,
}
impl StepPlan {
    pub(super) fn new(steps: Vec<Step>, before: usize, operation_state: Condition) -> Self {
        assert!(before <= steps.len());
        Self {
            steps: steps.into(),
            before,
            operation_state,
        }
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Frame {
    id: AccessExpressionId,
    temporaries: Vec<Holder>,
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct State {
    owned: BTreeMap<Holder, OwnedValue>,
    frames: Vec<Frame>,
}
impl State {
    pub(super) fn condition(&self) -> Condition {
        Condition {
            owned: self
                .owned
                .iter()
                .map(|(&holder, value)| (holder, value.missing.iter().cloned().collect()))
                .collect(),
        }
    }
}
/// One reachable ownership alternative. Private fields prevent unvalidated assembly.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Transition {
    before: State,
    plan: StepPlan,
    after: State,
}
impl Transition {
    #[must_use]
    pub const fn steps(&self) -> &[Step] {
        &self.plan.steps
    }
    #[must_use]
    pub fn before_steps(&self) -> &[Step] {
        &self.plan.steps[..self.plan.before]
    }
    #[must_use]
    pub fn after_steps(&self) -> &[Step] {
        &self.plan.steps[self.plan.before..]
    }
    #[must_use]
    pub fn condition(&self) -> Condition {
        self.before.condition()
    }
    /// Ownership after before-steps and before the selected operation starts.
    #[must_use]
    pub const fn operation_condition(&self) -> &Condition {
        &self.plan.operation_state
    }
    #[must_use]
    pub fn after_condition(&self) -> Condition {
        self.after.condition()
    }
    #[must_use]
    pub fn initialized_before(&self, holder: Holder) -> bool {
        self.before.initialized_root(holder)
    }
    #[must_use]
    pub fn initialized_after(&self, holder: Holder) -> bool {
        self.after.initialized_root(holder)
    }
}
/// Source-neutral transition alternatives keyed by the original operation graph.
#[derive(Clone, Debug, Default)]
pub struct Report {
    entry: Option<Condition>,
    transitions: BTreeMap<Location, BTreeSet<Transition>>,
}
impl Report {
    /// No entry evidence exists when initialization blocked resource analysis.
    #[must_use]
    pub const fn entry_condition(&self) -> Option<&Condition> {
        self.entry.as_ref()
    }
    pub(super) fn new(entry: Condition) -> Self {
        Self {
            entry: Some(entry),
            transitions: BTreeMap::new(),
        }
    }
    pub fn at(&self, at: Location) -> impl Iterator<Item = &Transition> {
        self.transitions.get(&at).into_iter().flatten()
    }
    pub(super) fn record(&mut self, at: Location, before: &State, plan: &StepPlan, after: &State) {
        self.transitions.entry(at).or_default().insert(Transition {
            before: before.clone(),
            plan: plan.clone(),
            after: after.clone(),
        });
    }
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, IntegrityError>;
/// Project the declaration-owned schema into its logical cleanup obligation.
/// This grants no initialized owner, acceptance, layout or native execution evidence.
/// # Errors
/// Rejects a type outside the supplied private program's supported payload domain.
pub fn payload(program: &Program, data: TypeId) -> Result<Payload> {
    let disposal = match program.data_schema(data).ok_or(INVALID)? {
        PayloadSchema::Builtin(BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Void) => {
            Disposal::Trivial
        }
        PayloadSchema::Builtin(BuiltinType::Error) => Disposal::OwnedError,
        PayloadSchema::Aggregate(_) => Disposal::Aggregate,
        PayloadSchema::Function(witness) => match witness.origin {
            nocter_model::ValueFunctionOrigin::Closure(_) => Disposal::OwnedCapture,
            nocter_model::ValueFunctionOrigin::Declaration(_) => Disposal::Trivial,
            nocter_model::ValueFunctionOrigin::Parameter(_) => Disposal::FunctionEnvironment,
            nocter_model::ValueFunctionOrigin::Result(_)
            | nocter_model::ValueFunctionOrigin::Application(_) => return Err(INVALID),
        },
        PayloadSchema::Builtin(_) => return Err(INVALID),
    };
    Ok(Payload { data, disposal })
}
pub(super) fn expression_payload(typed: &TypedQuery, id: AccessExpressionId) -> Result<Payload> {
    let Some(ty) = typed
        .body()
        .expressions()
        .get(id)
        .ok_or(INVALID)?
        .ty
        .normal()
    else {
        return Err(INVALID);
    };
    payload(typed.program(), ty.data())
}
