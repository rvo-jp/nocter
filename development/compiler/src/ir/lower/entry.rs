//! MIR-backed executable and source-test entry points.

use super::context::{ErrorPayloads, FunctionNames, FunctionSignatures, ResolvedSources};
use super::types::return_type_from_type_expr;
use crate::ast::{Block, FunctionDecl, TestDecl, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Type};
use crate::resolve::ResolveOutput;
use crate::source::{SourceId, SourceMap};
use crate::typecheck::TypedHir;

pub(super) fn lower_entry_function_with_target(
    function: &FunctionDecl,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        let span = function.generics.span.unwrap_or(function.parameters.span);
        return Err(vec![
            Diagnostic::error(
                "E8001",
                "native lowering can only lower a non-generic zero-parameter entry function",
            )
            .with_primary_span_if_absent(sources, span),
        ]);
    }
    let body = function.body.as_ref().ok_or_else(|| {
        vec![Diagnostic::error(
            "E8006",
            "native lowering cannot use a bodyless function as an entry point",
        )]
    })?;
    lower_entry_parts(
        &function.name,
        &function.return_type,
        body,
        target,
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        mir_bodies,
        resolved_sources,
        error_payloads,
    )
}

pub(super) fn lower_test_entry_function(
    test: &TestDecl,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    lower_entry_parts(
        &test.name,
        &test.return_type(),
        &test.body,
        target,
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        mir_bodies,
        resolved_sources,
        error_payloads,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_entry_parts(
    name: &str,
    return_type_expr: &TypeExpr,
    body: &Block,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let return_type =
        lower_entry_return_type(return_type_expr, resolved).map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    diagnostic.with_primary_span_if_absent(sources, return_type_expr.span())
                })
                .collect::<Vec<_>>()
        })?;
    let parameter_slots = super::parameter_slots::LoweringParameterSlots::default();
    let instructions = super::mir::lower_body(
        mir_bodies,
        body,
        &[],
        return_type_expr,
        &return_type,
        resolved,
        &resolved_sources,
        typed_hir,
        &std::collections::HashMap::new(),
        name,
        &function_signatures,
        &function_names,
        &error_payloads,
        &parameter_slots,
        root_source,
        sources,
    )?;
    Ok(Function {
        name: name.to_string(),
        target,
        return_type,
        instructions,
    })
}

fn lower_entry_return_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    let Some(return_type) = return_type_from_type_expr(ty, resolved) else {
        return Err(unsupported_entry_return_type_diagnostic());
    };
    if entry_return_type_is_supported(&return_type) {
        Ok(return_type)
    } else {
        Err(unsupported_entry_return_type_diagnostic())
    }
}

fn entry_return_type_is_supported(ty: &Type) -> bool {
    match ty {
        Type::I32 | Type::Usize | Type::Void => true,
        Type::Fallible(success) => entry_return_type_is_supported(success),
        _ => false,
    }
}

fn unsupported_entry_return_type_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8001",
        "native lowering can only lower entry function return type `i32`, `usize`, `i32!`, `usize!`, `void`, or `void!`",
    )]
}
