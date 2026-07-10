mod bindings;
mod context;
mod control_flow;
mod entry;
mod errors;
mod expressions;
mod functions;
mod imported_calls;
mod literals;
mod reachability;

#[cfg(test)]
mod tests;

use super::{CallTarget, Function, IrModule};
use crate::analysis::CompileUnitAnalysis;
use crate::ast::{FunctionDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Type;
use crate::resolve::ResolveOutput;
use crate::source::SourceId;
use context::FunctionSignatures;
use imported_calls::imported_call_diagnostics;
use reachability::same_file_call_targets;
use std::collections::{HashMap, HashSet};

pub(crate) fn lower_executable_with_entry(
    analysis: &CompileUnitAnalysis,
    entry_name: &str,
) -> Result<IrModule, Vec<Diagnostic>> {
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let Some(entry) = root.ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == entry_name => Some(function),
        _ => None,
    }) else {
        return Err(vec![Diagnostic::error(
            "E8000",
            format!("IR lowering requires entry function `{entry_name}`"),
        )]);
    };

    let root_functions = collect_root_functions(&root.ast.items);
    let diagnostics = imported_call_diagnostics(entry, root.ast.span.source, &root.resolved);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let function_signatures = collect_function_signatures(&root_functions);
    let mut functions = vec![entry::lower_entry_function(
        entry,
        function_signatures.clone(),
    )?];
    lower_reachable_functions(
        &mut functions,
        &root_functions,
        &function_signatures,
        entry_name,
        root.ast.span.source,
        &root.resolved,
    )?;

    Ok(IrModule::new(functions))
}

fn collect_root_functions(items: &[Item]) -> HashMap<&str, &FunctionDecl> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some((function.name.as_str(), function)),
            _ => None,
        })
        .collect()
}

fn collect_function_signatures(functions: &HashMap<&str, &FunctionDecl>) -> FunctionSignatures {
    FunctionSignatures::from_call_targets(
        functions
            .values()
            .filter_map(|function| {
                lower_signature_return_type(&function.return_type)
                    .map(|return_type| (CallTarget::same_file(function.name.clone()), return_type))
            })
            .collect(),
    )
}

fn lower_signature_return_type(ty: &TypeExpr) -> Option<Type> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        TypeExpr::Reference(reference) if reference.name == "void" => Some(Type::Void),
        TypeExpr::Fallible(fallible) => lower_signature_return_type(&fallible.success)
            .map(|success| Type::Fallible(Box::new(success))),
        _ => None,
    }
}

fn lower_reachable_functions(
    lowered: &mut Vec<Function>,
    candidates: &HashMap<&str, &FunctionDecl>,
    function_signatures: &FunctionSignatures,
    entry_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([entry_name.to_string()]);
    let mut queue = same_file_call_targets(&lowered[0]);

    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }

        let Some(function) = candidates.get(name.as_str()) else {
            return Err(vec![Diagnostic::error(
                "E8006",
                format!("IR v0 can only lower calls to same-file functions, got `{name}`"),
            )]);
        };
        let diagnostics = imported_call_diagnostics(function, root_source, resolved);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let function = functions::lower_function(function, function_signatures.clone())?;
        queue.extend(same_file_call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}
