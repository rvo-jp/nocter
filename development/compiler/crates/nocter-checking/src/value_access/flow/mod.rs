//! Shared evaluation/control, initialization, referent permissions and logical ownership.
//! An intermediate query is not complete whole-unit acceptance or native validity.
pub mod access;
mod build;
mod initialization;
mod projection;
pub mod resources;
#[cfg(test)]
mod tests;

use super::Query as TypedQuery;
pub use initialization::{
    CleanupFact, Initialization, InitializationRequirement, InitializationState,
    InitializationViolation, Location,
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, AccessFlowPointId, AccessLocalId, AccessPlaceId,
    AccessStatementId, Arena,
};
pub use projection::{Context, InitializationDiagnostic};
use std::sync::Arc;

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
    BooleanConstant {
        result: AccessExpressionId,
        value: bool,
    },
    /// A conditional's value is protected before the producing block exits.
    Forward {
        source: AccessExpressionId,
        result: AccessExpressionId,
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
        target: AccessPlaceId,
        value: AccessExpressionId,
    },
    CompoundAssign {
        statement: AccessStatementId,
        target: AccessPlaceId,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(AccessFlowPointId),
    Branch {
        condition: AccessExpressionId,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    },
    Return,
    Unreachable,
}
impl Terminator {
    #[must_use]
    pub fn successors(self) -> Vec<AccessFlowPointId> {
        match self {
            Self::Jump(next) => vec![next],
            Self::Branch { yes, no, .. } => vec![yes, no],
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
    graph: Graph,
    initialization: Initialization,
    access: access::Analysis,
}
impl Query {
    pub(in crate::value_access) fn materialize_constants(
        &mut self,
        values: &std::collections::BTreeMap<
            nocter_model::AccessCallableId,
            Arc<nocter_constant_evaluation::state::Value>,
        >,
        dependencies: &Arc<crate::value_access::source_dependencies::SourceDependencies>,
    ) -> Result<(), IntegrityError> {
        self.typed = Arc::new(super::evaluation::materialize(
            &self.typed,
            values,
            dependencies,
        )?);
        Ok(())
    }

    /// Builds control once and solves initialization, referent/access and ownership obligations.
    /// # Errors
    /// Returns an integrity failure if the closed typed product contains inconsistent identities.
    pub fn prepare(typed: Arc<TypedQuery>) -> Result<Self, IntegrityError> {
        if !typed.results_closed() {
            return Err(IntegrityError::UnclosedResultWitnesses);
        }
        Control::prepare(typed)?.finish()
    }
    #[must_use]
    pub fn typed(&self) -> &TypedQuery {
        &self.typed
    }
    #[must_use]
    pub const fn graph(&self) -> &Graph {
        &self.graph
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
    pub(super) graph: Graph,
}
impl Control {
    pub(super) fn prepare(typed: Arc<TypedQuery>) -> Result<Self, IntegrityError> {
        super::body_inputs::layout(&typed)?;
        let graph = build::build(typed.body())?;
        Ok(Self { typed, graph })
    }
    pub(super) fn finish(self) -> Result<Query, IntegrityError> {
        super::body_inputs::contract(&self.typed)?;
        if !self.typed.results_closed() {
            return Err(IntegrityError::UnclosedResultWitnesses);
        }
        super::results::validate(&self.typed, &self.graph)?;
        let initialization = initialization::analyze(&self.typed, &self.graph)?;
        let access = access::analyze(&self.typed, &self.graph, &initialization)?;
        Ok(Query {
            typed: self.typed,
            graph: self.graph,
            initialization,
            access,
        })
    }
}
