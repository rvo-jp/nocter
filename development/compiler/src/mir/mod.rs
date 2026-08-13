//! Checked control-flow representation between typed HIR and machine IR.

mod ids;
mod index;
mod lower;
mod model;
mod validate;

pub(crate) use ids::LocalId;
pub(crate) use index::BodyCache;
pub(crate) use lower::{BuildError, try_build_scalar_body};
pub(crate) use model::{
    BinaryOperator, Body, ComparisonOperator, LocalSource, Operand, Place, Rvalue, ScalarType,
    Statement, Terminator,
};
pub(crate) use validate::validate;
