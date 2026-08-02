use super::terminal::statement_guarantees_return_or_never;
use super::*;

mod block_effects;
mod expression_provenance;
mod input_sources;
mod propagation_collection;
mod summaries;
mod type_predicates;

pub(super) use block_effects::*;
pub(super) use expression_provenance::*;
pub(super) use input_sources::*;
pub(super) use propagation_collection::*;
pub(in crate::typecheck) use summaries::callable_provenance_summaries;
pub(in crate::typecheck) use type_predicates::type_contains_borrow_like;
pub(in crate::typecheck) use type_predicates::type_expr_contains_borrow_like;
