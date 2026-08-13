//! Checked control-flow representation between typed HIR and machine IR.

mod ids;
mod index;
mod lower;
mod model;
mod validate;

pub(crate) use ids::{BasicBlockId, LocalId};
pub(crate) use index::BodyCache;
pub(crate) use lower::{BuildError, try_build_scalar_body_with_return_mode};
pub(crate) use model::{
    BinaryOperator, Body, CallArgument, CallContinuation, ComparisonOperator, LocalSource,
    LoopRegion, Operand, Origin, Place, ReturnMode, Rvalue, ScalarType, Statement, Terminator,
};
pub(crate) use validate::validate;
