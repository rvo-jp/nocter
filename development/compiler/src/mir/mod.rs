//! Checked control-flow representation between typed HIR and machine IR.

mod ids;
mod lower;
mod model;
mod validate;

pub(crate) use lower::try_build_scalar_body;
pub(crate) use model::{Body, Operand, Place, Rvalue, Statement, Terminator};
pub(crate) use validate::validate;
