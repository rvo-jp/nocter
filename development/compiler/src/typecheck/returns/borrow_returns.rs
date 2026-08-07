use super::terminal::statement_guarantees_return_or_never;
use super::*;

mod block_effects;
mod expression_provenance;
mod input_sources;
mod literal_provenance;
mod mutation_effects;
mod propagation_collection;
mod result_allocation_evidence;
mod summaries;
mod summary_instantiation;
mod type_predicates;

pub(super) use block_effects::*;
pub(super) use expression_provenance::*;
pub(super) use input_sources::*;
pub(super) use literal_provenance::*;
pub(super) use mutation_effects::*;
pub(super) use propagation_collection::*;
pub(in crate::typecheck) use result_allocation_evidence::result_allocation_witness_for_callable_body;
pub(in crate::typecheck) use summaries::{
    borrow_return_provenance_for_callable_body, callable_provenance_summaries, function_summary_key,
};
pub(super) use summary_instantiation::*;
pub(in crate::typecheck) use type_predicates::returned_type_contains_readwrite_borrow;
pub(in crate::typecheck) use type_predicates::type_contains_borrow_like;
pub(in crate::typecheck) use type_predicates::type_expr_contains_borrow_like;
