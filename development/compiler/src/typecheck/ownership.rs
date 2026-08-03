use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_call_signature, resolved_method_for_call};
use super::copyability::{non_copy_owned_type_kind, non_copy_struct_type_name};
use super::diagnostics::{
    active_borrow_conflict_diagnostic, invalid_drop_target_diagnostic,
    overlapping_expression_borrow_diagnostic, uninitialized_binding_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_collection_for_binding,
    environment_for_for_range_binding, environment_for_function, environment_for_if_is_binding,
    environment_for_literal, environment_for_literal_pack_binding, environment_for_method,
    environment_for_parameters_in_impl, environment_for_switch_arm,
};
use super::expressions::{collection_builtin_call_type, expression_type};
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::provenance::{CallableId, CallableProvenanceSummaries, InputId};
use super::returns::{
    extend_terminal_lookahead_environment, returned_type_contains_readwrite_borrow,
    statement_evaluates_never_before_fallthrough, statement_guarantees_control_exit_or_never,
    type_contains_borrow_like,
};
use super::variants::switch_statement_covers_all_variants;
use crate::ast::{
    AstFile, Block, Expr, IdentifierExpr, ImplDecl, ImplMember, Item, MethodReceiverMode, Stmt,
    TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
mod borrow_collection;
mod borrow_conflicts;
mod entrypoints;
mod liveness;
mod model;
mod place_state;
mod places;
mod state_checks;

use borrow_collection::{
    collect_direct_borrow_expressions, collect_direct_borrow_expressions_in_statement,
    direct_borrow_source, returned_borrow_sources,
};
use borrow_conflicts::{
    check_expression_borrow_conflicts, check_statement_borrow_conflicts, record_statement_borrow,
};
pub(super) use entrypoints::check_ownership_states;
use entrypoints::{check_block_ownership, check_block_ownership_with_borrows};
use liveness::{expression_uses_identifier, statements_or_result_use_identifier_before_terminal};
use model::{
    ActiveBorrow, BorrowAction, BorrowPlace, DirectBorrowSource, FlowState, OwnershipState,
};
use place_state::{PlaceState, PlaceStateForest};
use places::{
    assignment_target_place, expression_place, expression_place_has_only_named_fields,
    index_expression_place, member_expression_place, owned_method_receiver_identifier,
    unwrap_group, whole_identifier,
};
use state_checks::{check_expression_ownership, check_statement_ownership};
