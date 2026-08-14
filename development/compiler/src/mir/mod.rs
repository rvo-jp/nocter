//! Checked control-flow representation between typed HIR and machine IR.

mod calls;
mod cleanup;
mod dataflow;
mod drop_obligations;
mod drop_plans;
mod ids;
mod index;
mod initialization;
mod loans;
mod locals;
mod lower;
mod model;
mod places;
mod scopes;
mod validate;

pub(crate) use calls::{CallInstance, CallInstanceKey, CallableIdentity, LiteralSegment};
pub(crate) use drop_plans::{DropPlan, DropPlanVariant};
pub(crate) use ids::{
    AllocationOverrideId, BasicBlockId, DropPlanId, LoanId, LocalId, ProjectionPathId, RegionId,
    ScopeId,
};
pub(crate) use index::BodyCache;
pub(crate) use locals::{
    Local, LocalOrigin, LocalStorage, OwnershipKind, ScalarType, ValueRepresentation, ViewKind,
};
pub(crate) use lower::{
    BuildError, BuildInputs, try_build_body_with_return_mode, try_build_closure_body,
    try_build_scalar_body_with_return_mode,
};
#[cfg(test)]
pub(crate) use model::BasicBlock;
pub(crate) use model::Loan;
pub(crate) use model::{
    AggregateElement, AggregateLeaf, AllocationContextOverride, AllocationRegion, BinaryOperator,
    Body, BorrowKind, CallArgument, CallContinuation, ComparisonOperator, Constant, LoopRegion,
    Operand, Origin, Place, ProjectionElement, ProjectionPath, ReturnMode, Rvalue, Statement,
    Terminator, UnaryOperator,
};
pub(crate) use scopes::Scope;
pub(crate) use validate::validate;

/// Completes construction-only MIR into the checked representation retained by
/// analysis and lowering. Cleanup insertion happens exactly once at this
/// boundary; consumers receive only the validated result.
pub(crate) fn finalize(mut body: Body) -> Result<Body, Vec<validate::ValidationError>> {
    let initialization_errors = initialization::validate(&body);
    if !initialization_errors.is_empty() {
        return Err(initialization_errors
            .into_iter()
            .map(validate::ValidationError::Initialization)
            .collect());
    }
    cleanup::materialize(&mut body);
    validate(&body)?;
    Ok(body)
}
