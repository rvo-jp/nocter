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
use crate::ast::{FunctionDecl, Item};
use crate::diagnostics::Diagnostic;
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

    let mut functions = vec![entry::lower_entry_function(entry)?];
    let root_functions = collect_root_functions(&root.ast.items);
    lower_reachable_functions(&mut functions, &root_functions, entry_name)?;

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

fn lower_reachable_functions(
    lowered: &mut Vec<Function>,
    candidates: &HashMap<&str, &FunctionDecl>,
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

        let function = functions::lower_function(function)?;
        queue.extend(call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

fn call_targets(function: &Function) -> VecDeque<String> {
    function
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::TailCall { function, .. } => Some(function.clone()),
            _ => None,
        })
        .collect()
}
