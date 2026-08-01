use super::*;

mod aggregate_bindings;
mod assignment_operators;
mod assignment_targets;
mod control_conditions;
mod drop_targets;
mod exit_analysis;
mod explicit_aggregate_moves;
mod outer_aggregate_moves;
mod runtime_shapes;
mod statement_diagnostics;

pub(super) use aggregate_bindings::*;
pub(super) use assignment_operators::*;
pub(super) use assignment_targets::*;
pub(super) use control_conditions::*;
pub(super) use drop_targets::*;
pub(super) use exit_analysis::*;
pub(super) use explicit_aggregate_moves::*;
pub(super) use outer_aggregate_moves::*;
pub(super) use runtime_shapes::*;
pub(super) use statement_diagnostics::*;
