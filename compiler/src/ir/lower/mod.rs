mod bindings;
mod context;
mod control_flow;
mod entry;
mod errors;
mod expressions;
mod functions;
mod literals;

#[cfg(test)]
mod tests;

use super::{Function, Instruction, IrModule};
use crate::analysis::CompileUnitAnalysis;
use crate::ast::{FunctionDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Type;
use context::FunctionSignatures;
use std::collections::{HashMap, HashSet, VecDeque};

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
    FunctionSignatures::new(
        functions
            .values()
            .filter_map(|function| {
                lower_signature_return_type(&function.return_type)
                    .map(|return_type| (function.name.clone(), return_type))
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
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([entry_name.to_string()]);
    let mut queue = call_targets(&lowered[0]);

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

        let function = functions::lower_function(function, function_signatures.clone())?;
        queue.extend(call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

fn call_targets(function: &Function) -> VecDeque<String> {
    let mut targets = VecDeque::new();
    collect_call_targets(&function.instructions, &mut targets);
    targets
}

fn collect_call_targets(instructions: &[Instruction], targets: &mut VecDeque<String>) {
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { function, .. }
            | Instruction::CallBool { function, .. }
            | Instruction::TailCall { function, .. } => targets.push_back(function.clone()),
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                collect_call_targets(then_instructions, targets);
                collect_call_targets(else_instructions, targets);
            }
            Instruction::WriteStaticStderr(_)
            | Instruction::SetI32 { .. }
            | Instruction::SetBool { .. }
            | Instruction::AddI32 { .. }
            | Instruction::Return => {}
        }
    }
}
