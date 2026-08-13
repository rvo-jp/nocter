//! Checked control-flow representation between typed HIR and machine IR.

mod cleanup;
mod dataflow;
mod drop_obligations;
mod ids;
mod index;
mod initialization;
mod loans;
mod locals;
mod lower;
mod model;
mod scopes;
mod validate;

pub(crate) use ids::{BasicBlockId, LoanId, LocalId, ScopeId};
pub(crate) use index::BodyCache;
pub(crate) use locals::{
    Local, LocalOrigin, LocalStorage, OwnershipKind, ScalarType, ValueRepresentation,
};
pub(crate) use lower::{BuildError, try_build_scalar_body_with_return_mode};
#[cfg(test)]
pub(crate) use model::Loan;
#[cfg(test)]
pub(crate) use model::{BasicBlock, Constant};
pub(crate) use model::{
    BinaryOperator, Body, BorrowKind, CallArgument, CallContinuation, ComparisonOperator,
    LoopRegion, Operand, Origin, Place, ReturnMode, Rvalue, Statement, Terminator,
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
