//! Finite logical ownership and sparse partial-payload cleanup for the closed data contracts.
//! These transitions retain obligations; they do not choose physical storage or an ABI.
use super::{IntegrityError, Location};
use crate::value_access::{Argument, Query as TypedQuery};
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_model::{
    AccessExpressionId, AccessLocalId, BuiltinType, FieldPath, TypeId, ValueInputOperation,
};
use std::collections::{BTreeMap, BTreeSet};

mod assembly;
pub(super) use assembly::Assembly;
mod capture;
mod function_view;
mod missing;
pub use missing::Missing;
mod path;
mod payload;
mod progression;
pub use path::{ArrayFrontier, Index, IndexBacking, Path, RetainedIndex};
mod backing;
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
    /// An ordinary usize payload whose lifetime follows a retained element hole.
    IndexSnapshot(RetainedIndex),
    /// A changing last-committed endpoint, tied to one missing prefix rather than a point.
    ArrayFrontier(ArrayFrontier),
    Return,
}
impl Holder {
    pub(super) const fn is_backing(self) -> bool {
        matches!(self, Self::IndexSnapshot(_) | Self::ArrayFrontier(_))
    }
    #[must_use]
    pub fn place(self) -> Place {
        Place {
            holder: self,
            fields: Path::default(),
        }
    }
}

/// A logical selection within one resource holder, not a physical address or an access grant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Place {
    holder: Holder,
    fields: Path,
}

#[derive(Debug)]
pub(super) enum CleanupFailure {
    Integrity(IntegrityError),
    IncompleteDestructor(Place),
}

impl From<IntegrityError> for CleanupFailure {
    fn from(error: IntegrityError) -> Self {
        Self::Integrity(error)
    }
}
impl Place {
    #[must_use]
    pub fn with_fields(self, fields: Path) -> Self {
        Self { fields, ..self }
    }
    #[must_use]
    pub const fn holder(&self) -> Holder {
        self.holder
    }
    #[must_use]
    pub const fn fields(&self) -> &Path {
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
            fields: self.fields.concat_fields(suffix),
        }
    }
    #[must_use]
    pub fn project_retained(&self, index: RetainedIndex) -> Self {
        Self {
            holder: self.holder,
            fields: self.fields.project_retained(index),
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
    /// A closed callable domain; its actual tag selects one complete case's cleanup.
    CallableAlternatives,
    OwnedCapture,
    /// The supplied environment's destructor, retained symbolically until witness specialization.
    FunctionEnvironment,
    Trivial,
    OwnedError,
    /// One exact producer whose activation has not executed any body operation.
    UnstartedFuture(nocter_model::AccessCodeId),
    /// A structural container; its declaration schema selects component or homogeneous cleanup.
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
}

/// A closed input whose descriptor suffices for local activation storage.
/// Edits require the same initialized representation at completion, with no dependent
/// text or callable channels to write back.
pub(in crate::value_access) fn local_future_input(
    program: &Program,
    data: TypeId,
    operation: ValueInputOperation,
) -> Option<Payload> {
    let key = program.instantiate_closed_data(data).ok()?;
    if !key.is_context_free() {
        return None;
    }
    let domain = if operation == ValueInputOperation::Edit {
        PayloadDomain::LocalFutureEdit
    } else {
        PayloadDomain::Ownership
    };
    let payload = payload_in(program, data, domain).ok()?;
    matches!(
        payload.disposal(),
        Disposal::Trivial | Disposal::OwnedError | Disposal::Aggregate
    )
    .then_some(payload)
}

pub(super) struct FutureActivation<'a> {
    pub payload: Payload,
    pub inputs: Box<[(&'a Argument, Payload)]>,
}

/// The exact prepared source committed to a local activation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ActivationInput<P> {
    Take { source: Place, payload: P },
    Access(ActivationAccess),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ActivationAccess {
    pub source: ActivationAccessSource,
    pub data: TypeId,
    pub operation: ValueInputOperation,
    pub region: super::access::Region,
    pub created: Location,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ActivationAccessSource {
    Existing(crate::value_access::Referent),
    Temporary {
        call: AccessExpressionId,
        index: usize,
    },
}

impl ActivationAccess {
    pub(in crate::value_access::flow) fn retained_access(&self) -> super::access::RetainedAccess {
        use super::access::{Creation, RetainedAccess, RetentionKind};
        RetainedAccess {
            created: Creation::Action(self.created),
            kind: RetentionKind::Access(self.operation.access_mode()),
            required: None,
        }
    }
}

impl ActivationInput<Payload> {
    #[must_use]
    pub fn contract(&self) -> nocter_declarations::value_access::FunctionInput {
        let (data, operation) = match self {
            Self::Take { payload, .. } => (payload.data(), ValueInputOperation::Take),
            Self::Access(access) => (access.data, access.operation),
        };
        nocter_declarations::value_access::FunctionInput { data, operation }
    }
}

/// Form the ownership receipt for a selected producer and its represented inputs.
/// The checked unit must separately establish the producer's effects and backing.
pub(super) fn future_activation(
    typed: &TypedQuery,
    call: AccessExpressionId,
) -> Result<FutureActivation<'_>> {
    use crate::value_access::{CallKind, ExpressionKind};
    use nocter_declarations::CallableExecution;
    use nocter_model::{ValueAccessMode, ValueFunctionOrigin};

    let expression = typed.body().expressions().get(call).ok_or(INVALID)?;
    let ExpressionKind::Call {
        kind: CallKind::Direct,
        target: ValueFunctionOrigin::Declaration(producer),
        arguments,
    } = &expression.kind
    else {
        return Err(INVALID);
    };
    let program = typed.program();
    let declaration = program.codes().get(*producer).ok_or(INVALID)?;
    let Some(CallableExecution::Deferred { output }) =
        program.function_execution(ValueFunctionOrigin::Declaration(*producer))
    else {
        return Err(INVALID);
    };
    let data = declaration.result();
    if arguments.len() != declaration.parameters().len()
        || expression.ty.normal()
            != Some(nocter_model::ValueAccess::new(data, ValueAccessMode::Own))
        || program.data_schema(data) != Some(PayloadSchema::Future(output))
    {
        return Err(INVALID);
    }
    let inputs = arguments
        .iter()
        .zip(declaration.parameters())
        .map(|(argument, &id)| {
            let parameter = program.parameters().get(id).ok_or(INVALID)?;
            if parameter.operation() != argument.operation {
                return Err(INVALID);
            }
            Ok((
                argument,
                local_future_input(program, parameter.data(), parameter.operation())
                    .ok_or(INVALID)?,
            ))
        })
        .collect::<Result<_>>()?;
    Ok(FutureActivation {
        payload: Payload {
            data,
            disposal: Disposal::UnstartedFuture(*producer),
        },
        inputs,
    })
}
mod step;
mod transport;
pub use step::{PayloadSite, Step};

/// Exact owning-container presence and sparse missing selections at one operation boundary.
/// This is a checked guard projection, not an access grant or a physical drop-flag layout.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Condition {
    owned: Box<[(Holder, Box<[Missing]>)]>,
}
impl Condition {
    /// Listed holders carry an owning container; unlisted holders carry no ownership.
    /// Scoped input storage is independent. Missing paths stay prefix-free.
    pub fn owned(&self) -> impl Iterator<Item = (Holder, &[Missing])> {
        self.owned
            .iter()
            .map(|(holder, missing)| (*holder, missing.as_ref()))
    }
}

mod instance;
pub use instance::InstantiatedPayload;

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
    code: Option<nocter_model::ValueCode>,
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
    /// Original sparse ownership at entry. None means no owning carrier, not an empty one.
    #[must_use]
    pub fn missing_before(
        &self,
        holder: Holder,
    ) -> Option<impl ExactSizeIterator<Item = &Missing>> {
        self.before
            .owned
            .get(&holder)
            .map(|value| value.missing.iter())
    }
    /// Original sparse ownership on normal exit, independently of source completion metadata.
    #[must_use]
    pub fn missing_after(&self, holder: Holder) -> Option<impl ExactSizeIterator<Item = &Missing>> {
        self.after
            .owned
            .get(&holder)
            .map(|value| value.missing.iter())
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
    payload_in(program, data, PayloadDomain::Ownership)
}

#[derive(Clone, Copy)]
enum PayloadDomain {
    Ownership,
    LocalFutureEdit,
}

fn payload_in(program: &Program, data: TypeId, domain: PayloadDomain) -> Result<Payload> {
    // An aggregate or capture descriptor cannot hide an unadmitted descendant. Validate shared
    // schemas once before granting even the root cleanup obligation.
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = vec![(data, false)];
    let mut disposal = None;
    while let Some((current, guarded)) = pending.pop() {
        if !seen.insert((current, guarded)) {
            continue;
        }
        let schema = program.data_schema(current).ok_or(INVALID)?;
        if matches!(domain, PayloadDomain::LocalFutureEdit)
            && matches!(
                schema,
                PayloadSchema::Builtin(BuiltinType::Str) | PayloadSchema::Function(_)
            )
        {
            return Err(INVALID);
        }
        let selected = match schema {
            PayloadSchema::Builtin(BuiltinType::Bool | BuiltinType::Void | BuiltinType::Str) => {
                Disposal::Trivial
            }
            PayloadSchema::Enumeration(schema) => {
                if schema.has_payload() {
                    for &variant in schema.variants() {
                        pending.extend(
                            program
                                .variant_components(current, variant)
                                .ok_or(INVALID)?
                                .map(|field| (field.data, guarded)),
                        );
                    }
                    Disposal::Aggregate
                } else {
                    Disposal::Trivial
                }
            }
            PayloadSchema::Builtin(builtin)
                if nocter_model::BuiltinNumericType::for_builtin(builtin)
                    .and_then(nocter_model::BuiltinNumericType::integer_bounds)
                    .is_some() =>
            {
                Disposal::Trivial
            }
            PayloadSchema::Builtin(BuiltinType::Error) => Disposal::OwnedError,
            PayloadSchema::Optional(payload) => {
                pending.push((payload, true));
                Disposal::Aggregate
            }
            PayloadSchema::Fallible(payload) => {
                pending.push((payload, true));
                pending.push((program.data_type(BuiltinType::Error).ok_or(INVALID)?, true));
                Disposal::Aggregate
            }
            PayloadSchema::Aggregate(schema) => {
                pending.extend(
                    schema
                        .components()
                        .map(|component| (component.data, guarded)),
                );
                Disposal::Aggregate
            }
            PayloadSchema::Array(array) => {
                if !array.is_empty() {
                    pending.push((array.element(), guarded));
                }
                Disposal::Aggregate
            }
            PayloadSchema::Function(witness) => match witness.origin {
                nocter_model::ValueFunctionOrigin::Closure(id) => {
                    pending.push((program.closure(id).ok_or(INVALID)?.environment(), guarded));
                    Disposal::OwnedCapture
                }
                nocter_model::ValueFunctionOrigin::Declaration(_) => Disposal::Trivial,
                nocter_model::ValueFunctionOrigin::Parameter(_)
                | nocter_model::ValueFunctionOrigin::Contract(_) => Disposal::FunctionEnvironment,
                nocter_model::ValueFunctionOrigin::Result(_)
                | nocter_model::ValueFunctionOrigin::Application(_)
                    if guarded =>
                {
                    Disposal::FunctionEnvironment
                }
                nocter_model::ValueFunctionOrigin::Result(_)
                | nocter_model::ValueFunctionOrigin::Application(_) => return Err(INVALID),
            },
            PayloadSchema::Builtin(_) | PayloadSchema::Future(_) => {
                return Err(INVALID);
            }
        };

        if current == data {
            disposal = Some(selected);
        }
    }
    let disposal = disposal.ok_or(INVALID)?;
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
