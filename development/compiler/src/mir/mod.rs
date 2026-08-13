//! Checked control-flow representation between typed HIR and machine IR.

mod ids;
mod index;
mod locals;
mod lower;
mod model;
mod scopes;
mod validate;

pub(crate) use ids::{BasicBlockId, LocalId, ScopeId};
pub(crate) use index::BodyCache;
pub(crate) use locals::{Local, LocalOrigin, LocalStorage, ScalarType};
pub(crate) use lower::{BuildError, try_build_scalar_body_with_return_mode};
#[cfg(test)]
pub(crate) use model::{BasicBlock, Constant};
pub(crate) use model::{
    BinaryOperator, Body, CallArgument, CallContinuation, ComparisonOperator, LoopRegion, Operand,
    Origin, Place, ReturnMode, Rvalue, Statement, Terminator,
};
pub(crate) use scopes::Scope;
pub(crate) use validate::validate;
