//! Checked control-flow representation between typed HIR and machine IR.

mod ids;
mod lower;
mod model;
mod validate;

pub(crate) use lower::try_build_scalar_literal_body;
pub(crate) use model::{Body, Operand, Rvalue, Statement, Terminator};
pub(crate) use validate::validate;
