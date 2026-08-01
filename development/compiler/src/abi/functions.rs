use super::types::{
    abi_return_from_type_expr, abi_value_from_type_expr_inner, resolved_for_type_expr,
    type_symbol_by_reference_name,
};
use super::{AbiParameter, AbiTypeError, FunctionAbi, LayoutError, ReturnPassing};
use crate::ast::TypeExpr;
use crate::resolve::{FunctionSignature, ResolveOutput};
use crate::source::SourceId;
use std::collections::HashSet;

pub fn function_abi_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<FunctionAbi, AbiTypeError> {
    function_abi_from_signature_with_resolver(signature, resolved, |_| Some(resolved))
}

pub fn function_abi_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<FunctionAbi, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let parameters =
        function_parameters_abi_from_signature_inner(signature, fallback_resolved, &resolver)?;
    let return_value =
        abi_return_from_type_expr(&signature.return_type, fallback_resolved, &resolver)?;

    Ok(FunctionAbi {
        parameters,
        return_value,
    })
}

pub fn function_parameters_abi_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<Vec<AbiParameter>, AbiTypeError> {
    function_parameters_abi_from_signature_with_resolver(signature, resolved, |_| Some(resolved))
}

pub fn function_parameters_abi_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<Vec<AbiParameter>, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    function_parameters_abi_from_signature_inner(signature, fallback_resolved, &resolver)
}

fn function_parameters_abi_from_signature_inner<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<Vec<AbiParameter>, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    signature
        .parameters
        .iter()
        .map(|parameter| {
            Ok(AbiParameter {
                name: parameter.name.clone(),
                value: abi_value_from_type_expr_inner(&parameter.ty, fallback_resolved, resolver)?,
            })
        })
        .collect()
}

pub fn function_parameter_abi_word_count_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<usize, AbiTypeError> {
    function_parameter_abi_word_count_from_signature_with_resolver(signature, resolved, |_| {
        Some(resolved)
    })
}

pub fn function_parameter_abi_word_count_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<usize, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let mut count = 0_usize;
    for parameter in &signature.parameters {
        if type_expr_resolves_to_error(
            &parameter.ty,
            fallback_resolved,
            &resolver,
            &mut HashSet::new(),
        ) {
            count = count
                .checked_add(4)
                .ok_or(AbiTypeError::Layout(LayoutError::SizeOverflow))?;
            continue;
        }

        let parameter = AbiParameter {
            name: parameter.name.clone(),
            value: abi_value_from_type_expr_inner(&parameter.ty, fallback_resolved, &resolver)?,
        };
        count = count
            .checked_add(parameter.value.parameter_abi_word_count())
            .ok_or(AbiTypeError::Layout(LayoutError::SizeOverflow))?;
    }
    Ok(count)
}

fn type_expr_resolves_to_error<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let TypeExpr::Reference(reference) = ty else {
        return false;
    };

    if reference.name == "error" {
        return true;
    }

    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
        return false;
    };
    let Some(target) = &symbol.alias_target else {
        return false;
    };
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }
    let result = type_expr_resolves_to_error(target, fallback_resolved, resolver, resolving_names);
    resolving_names.remove(&symbol.canonical_name);
    result
}

pub fn function_success_return_passing_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<ReturnPassing, AbiTypeError> {
    function_success_return_passing_from_signature_with_resolver(signature, resolved, |_| {
        Some(resolved)
    })
}

pub fn function_success_return_passing_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<ReturnPassing, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    top_level_success_return_passing_from_type_expr(
        &signature.return_type,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
    )
}

fn top_level_success_return_passing_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Result<ReturnPassing, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return abi_return_from_type_expr(ty, fallback_resolved, resolver)
                    .map(|return_value| return_value.passing());
            };
            let Some(target) = &symbol.alias_target else {
                return abi_return_from_type_expr(ty, fallback_resolved, resolver)
                    .map(|return_value| return_value.passing());
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
            }
            let result = top_level_success_return_passing_from_type_expr(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => {
            abi_return_from_type_expr(&fallible.success, fallback_resolved, resolver)
                .map(|return_value| return_value.passing())
        }
        TypeExpr::Optional(optional) => {
            abi_return_from_type_expr(&optional.inner, fallback_resolved, resolver)
                .map(|return_value| return_value.passing())
        }
        _ => abi_return_from_type_expr(ty, fallback_resolved, resolver)
            .map(|return_value| return_value.passing()),
    }
}
