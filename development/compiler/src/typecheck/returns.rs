use super::TypecheckSource;
use super::bindings::continuing_binding_type;
use super::calls::{
    call_return_type, method_member_for_call, resolved_call_signature, resolved_method_for_call,
};
use super::copyability::implicit_non_copy_owned_value_source;
use super::diagnostics::{
    body_result_type_mismatch_diagnostic, borrow_return_escapes_diagnostic,
    catch_block_fallthrough_diagnostic, fallible_success_error_diagnostic,
    missing_return_diagnostic, missing_return_value_diagnostic, never_return_statement_diagnostic,
    non_copy_struct_return_diagnostic, return_type_mismatch_diagnostic,
    unexpected_body_result_diagnostic, unexpected_return_value_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_method, environment_for_parameters_in_impl,
    environment_for_switch_arm, impl_member_name,
};
use super::expressions::expression_type;
use super::fallible::{check_catch_operand, check_propagation};
use super::model::{
    CallableKind, ReturnContext, Type, TypeEnvironment, binding_kind_is_mutable, same_known_type,
};
use super::numeric::integer_literal_expr_value;
use super::operations::is_expression_assignable;
use super::type_expr::{type_expr_to_type_in_environment, type_expr_to_type_with_substitutions};
use super::variants::{is_enum_variant_call, switch_statement_covers_all_variants};
use crate::ast::{
    AstFile, Block, Expr, IfIsStmt, ImplDecl, ImplMember, InterpolatedStringPart, Item,
    PropagationExpr, ReturnStmt, Stmt, SwitchArm, SwitchPayloadBinding, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{LocalSymbolKind, ResolveOutput, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

mod borrow_returns;
mod return_checks;
mod terminal;
mod utility;

use borrow_returns::*;
use return_checks::*;
use utility::*;

pub(super) use terminal::{
    block_guarantees_control_exit_or_never, block_guarantees_return_or_never,
    extend_terminal_lookahead_environment, statement_evaluates_never_before_fallthrough,
    statement_guarantees_control_exit_or_never,
};

pub(super) fn check_return_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summary_sources: &[TypecheckSource<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let summaries = borrow_return_summaries(summary_sources);
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let mut environment = environment_for_function(function, resolved);
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                let context = ReturnContext::new(
                    if function.owner.is_some() {
                        CallableKind::AssociatedFunction(function.name.clone())
                    } else {
                        CallableKind::Function(function.name.clone())
                    },
                    type_expr_to_type_in_environment(&function.return_type, resolved, &environment),
                    function.return_type.span(),
                );
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    &function.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    &summaries,
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_return_types(sources, impl_, resolved, diagnostics, &summaries);
            }
            _ => {}
        }
    }
}
