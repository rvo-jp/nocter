//! Shared evaluation/control, initialization, referent permissions and logical ownership.
//! An intermediate query is not complete whole-unit acceptance or native validity.
pub mod access;
mod aliases;
pub(super) use aliases::Aliases;
mod build;
pub(super) mod call_writes;
mod initialization;
mod numeric;
mod outcome_domains;
pub(in crate::value_access) use outcome_domains::{OutcomeSource, assertion_source};
mod projection;
pub mod resources;
#[cfg(test)]
mod tests;

use super::{Query as TypedQuery, Referent};
pub use initialization::{
    Initialization, InitializationRequirement, InitializationState, InitializationViolation,
    Location,
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, AccessFlowPointId, AccessLocalId, AccessStatementId, Arena,
};
pub use numeric::NumericFacts;
pub use projection::{Context, InitializationDiagnostic};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

fn owned_base(typed: &TypedQuery, source: Referent) -> bool {
    source
        .static_base(
            |id| typed.body().expressions().get(id),
            |id| typed.body().blocks().get(id),
        )
        .and_then(|id| typed.body().places().get(id))
        .and_then(|place| typed.body().locals().get(place.local))
        .is_some_and(|local| local.access().mode() == nocter_model::ValueAccessMode::Own)
}

fn cancelled_slots(
    typed: &TypedQuery,
    call: AccessExpressionId,
    prepared: usize,
) -> Result<impl DoubleEndedIterator<Item = usize> + '_, IntegrityError> {
    let prefix = typed
        .body()
        .call_arguments(call)
        .and_then(|arguments| arguments.prepared_prefix(prepared))
        .ok_or(IntegrityError::InconsistentTypedBody)?;
    Ok(prefix.map(|(slot, _)| slot))
}

/// Source-neutral diagnostic subject in the owning typed query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    Expression(AccessExpressionId),
    Statement(AccessStatementId),
    Block(AccessBlockId),
}

/// Evaluation steps are ordered. Call preparation refers to the already selected typed plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// The evaluated index is known to fail; no selected value/address or owner is supplied.
    IndexFailure(AccessExpressionId),
    BeginEvaluation(AccessExpressionId),
    EndEvaluation(AccessExpressionId),
    Evaluate(AccessExpressionId),
    /// Read a scalar operand once, before evaluating a later operand.
    PrepareOperand {
        expression: AccessExpressionId,
        operand: usize,
        value: AccessExpressionId,
    },
    Condition(AccessExpressionId),
    InspectEnum {
        source: AccessExpressionId,
        operation: nocter_model::ValueInputOperation,
    },
    BooleanConstant {
        result: AccessExpressionId,
        value: bool,
    },
    /// A conditional's value is protected before the producing block exits.
    Forward {
        source: AccessExpressionId,
        result: AccessExpressionId,
        scope_exit: bool,
    },
    /// Short-circuit Boolean answers copy the selected payload into a fresh X result.
    BooleanAnswer {
        source: AccessExpressionId,
        result: AccessExpressionId,
    },
    PrepareArgument {
        call: AccessExpressionId,
        argument: usize,
    },
    CommitCall(AccessExpressionId),
    /// Commits future construction at this call site. The deferred body is not invoked.
    ConstructFuture(AccessExpressionId),
    /// Consume one activation and receive its output only after a normal drive.
    DriveFuture {
        future: AccessExpressionId,
        result: AccessExpressionId,
    },
    /// Abrupt completion cleans only the successfully prepared prefix; no invocation occurred.
    CancelCall {
        call: AccessExpressionId,
        prepared: usize,
    },
    Bind {
        local: AccessLocalId,
        value: AccessExpressionId,
    },
    Assign {
        target: Referent,
        value: AccessExpressionId,
    },
    CompoundAssign {
        statement: AccessStatementId,
        target: Referent,
        value: AccessExpressionId,
    },
    Discard(AccessExpressionId),
    Drop(AccessLocalId),
    StageReturn(AccessExpressionId),
    /// Reverse binding order, including parameter holders at the root. Drop only live owned slots.
    ExitScope {
        block: AccessBlockId,
        locals: Box<[AccessLocalId]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub origin: Origin,
    pub operation: Operation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(AccessFlowPointId),
    Branch {
        condition: AccessExpressionId,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    },
    EnumSwitch {
        source: AccessExpressionId,
        cases: Arc<[(u64, AccessFlowPointId)]>,
        fallback: AccessFlowPointId,
    },
    Return,
    Unreachable,
}
impl Terminator {
    #[must_use]
    pub fn successors(&self) -> Vec<AccessFlowPointId> {
        match self {
            Self::Jump(next) => vec![*next],
            Self::Branch { yes, no, .. } => vec![*yes, *no],
            Self::EnumSwitch {
                cases, fallback, ..
            } => cases
                .iter()
                .map(|&(_, point)| point)
                .chain(std::iter::once(*fallback))
                .collect(),
            Self::Return | Self::Unreachable => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Point {
    pub actions: Box<[Action]>,
    pub terminator: Terminator,
}

/// Immutable control/evaluation authority. All identities are scoped to the query that owns it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Graph {
    entry: AccessFlowPointId,
    points: Arena<AccessFlowPointId, Point>,
}
impl Graph {
    /// Structural reachability of the prepared graph. Detached completion points may retain
    /// unproduced input plans and never supply executable effects.
    pub(in crate::value_access) fn reachable(
        &self,
    ) -> Result<BTreeSet<AccessFlowPointId>, IntegrityError> {
        let mut pending = BTreeSet::from([self.entry()]);
        let mut reached = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            if reached.insert(id) {
                let point = self
                    .points()
                    .get(id)
                    .ok_or(IntegrityError::InconsistentTypedBody)?;
                pending.extend(point.terminator.successors());
            }
        }
        Ok(reached)
    }
    #[must_use]
    pub const fn entry(&self) -> AccessFlowPointId {
        self.entry
    }
    #[must_use]
    pub const fn points(&self) -> &Arena<AccessFlowPointId, Point> {
        &self.points
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityError {
    InconsistentTypedBody,
    UnclosedResultWitnesses,
}

/// Shared typed authority, its control graph and fixed-point slot facts. Not a checked program.
#[derive(Clone, Debug)]
pub struct Query {
    typed: Arc<TypedQuery>,
    graph: Arc<Graph>,
    numeric: numeric::Analysis,
    initialization: Initialization,
    access: access::Analysis,
}
impl Query {
    /// Builds control once and solves initialization, referent/access and ownership obligations.
    /// # Errors
    /// Returns an integrity failure if the closed typed product contains inconsistent identities.
    pub fn prepare(typed: Arc<TypedQuery>) -> Result<Self, IntegrityError> {
        if !typed.results_closed() {
            return Err(IntegrityError::UnclosedResultWitnesses);
        }
        let control = Control::prepare(typed)?;
        let writes = call_writes::Catalog::prepare([&control])?;
        control.finish(&writes)
    }
    #[must_use]
    pub fn typed(&self) -> &TypedQuery {
        &self.typed
    }
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Numeric relationships before a reached operation. These grant no access or bounds waiver.
    #[must_use]
    pub fn numeric_at(&self, location: Location) -> Option<NumericFacts> {
        self.numeric.at(&self.typed, &self.graph, location)
    }
    #[must_use]
    pub const fn initialization(&self) -> &Initialization {
        &self.initialization
    }
    #[must_use]
    pub const fn access(&self) -> &access::Analysis {
        &self.access
    }
}

/// Control is prepared once, independently of unresolved value supply. Initialization and ownership
/// consume this graph only after result identities and supply close; no public constructor exists.
#[derive(Clone, Debug)]
pub(super) struct Control {
    pub(super) typed: Arc<TypedQuery>,
    pub(super) graph: Arc<Graph>,
}
/// One lexical code index for consumers of root bodies and their captured code.
pub(super) fn scope<'a>(
    controls: impl IntoIterator<Item = &'a Control>,
) -> Result<BTreeMap<nocter_model::ValueCode, &'a Control>, IntegrityError> {
    let mut scope = BTreeMap::new();
    let mut pending: Vec<_> = controls.into_iter().collect();
    while let Some(control) = pending.pop() {
        if let Some(previous) = scope.insert(control.typed.code(), control) {
            if !Arc::ptr_eq(&previous.typed, &control.typed) {
                return Err(IntegrityError::InconsistentTypedBody);
            }
            continue;
        }
        for (&id, nested) in &control.typed.closures {
            if nested.typed.code() != nocter_model::ValueCode::Closure(id) {
                return Err(IntegrityError::InconsistentTypedBody);
            }
            pending.push(nested);
        }
    }
    Ok(scope)
}
impl Control {
    pub(super) fn prepare(typed: Arc<TypedQuery>) -> Result<Self, IntegrityError> {
        super::body_inputs::layout(&typed)?;
        let graph = Arc::new(build::build(&typed)?);
        let mut control = Self { typed, graph };
        if control.typed.results_closed() {
            control.refine_outcome_domains()?;
        }
        Ok(control)
    }
    pub(super) fn finish(self, writes: &call_writes::Catalog) -> Result<Query, IntegrityError> {
        super::body_inputs::contract(&self.typed)?;
        if !self.typed.results_closed() {
            return Err(IntegrityError::UnclosedResultWitnesses);
        }
        super::results::validate(&self.typed, &self.graph)?;
        let numeric = numeric::analyze(&self.typed, &self.graph, writes.calls(&self)?)?;
        let initialization = initialization::analyze(&self.typed, &self.graph, &numeric)?;
        let access = access::analyze(&self.typed, &self.graph, &initialization, &numeric)?;
        Ok(Query {
            typed: self.typed,
            graph: self.graph,
            numeric,
            initialization,
            access,
        })
    }
}
