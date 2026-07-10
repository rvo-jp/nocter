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
use reachability::reachable_call_targets;
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

    let function_index = FunctionIndex::new(analysis, root.ast.span.source);
    let diagnostics = imported_call_diagnostics(entry, root.ast.span.source, &root.resolved);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let function_signatures = function_index.signatures();
    let mut functions = vec![entry::lower_entry_function(
        entry,
        function_signatures.clone(),
        root.ast.span.source,
        &root.resolved,
    )?];
    lower_reachable_functions(
        &mut functions,
        &function_index,
        &function_signatures,
        entry_name,
        root.ast.span.source,
        &root.resolved,
    )?;

    Ok(IrModule::new(functions))
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
    function_index: &FunctionIndex<'_>,
    function_signatures: &FunctionSignatures,
    entry_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([CallTarget::same_file(entry_name)]);
    let mut queue = reachable_call_targets(&lowered[0]);

    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }

        let CallTarget::SameFile(name) = &target else {
            continue;
        };
        let Some(function) = function_index.same_file_definition(name) else {
            return Err(vec![Diagnostic::error(
                "E8006",
                format!("IR v0 can only lower calls to same-file functions, got `{name}`"),
            )]);
        };
        let diagnostics = imported_call_diagnostics(function, root_source, resolved);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let function = functions::lower_function(
            function,
            function_signatures.clone(),
            root_source,
            resolved,
        )?;
        queue.extend(reachable_call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

struct FunctionIndex<'a> {
    definitions: HashMap<CallTarget, &'a FunctionDecl>,
}

impl<'a> FunctionIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        for file in &analysis.files {
            for item in &file.ast.items {
                let Item::Function(function) = item else {
                    continue;
                };
                let target = if file.ast.span.source == root_source {
                    CallTarget::same_file(function.name.clone())
                } else {
                    CallTarget::imported(file.ast.span.source, function.name.clone())
                };
                definitions.insert(target, function);
            }
        }
        Self { definitions }
    }

    fn same_file_definition(&self, name: &str) -> Option<&'a FunctionDecl> {
        self.definitions.get(&CallTarget::same_file(name)).copied()
    }

    fn signatures(&self) -> FunctionSignatures {
        FunctionSignatures::from_call_targets(
            self.definitions
                .iter()
                .filter_map(|(target, function)| {
                    lower_signature_return_type(&function.return_type)
                        .map(|return_type| (target.clone(), return_type))
                })
                .collect(),
        )
    }
}
