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

pub(crate) fn lower_program(analysis: &CompileUnitAnalysis) -> Result<IrModule, Vec<Diagnostic>> {
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let Some(program) = root.ast.items.iter().find_map(|item| match item {
        Item::Program(program) => Some(program),
        _ => None,
    }) else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a `program` entry",
        )]);
    };

    let mut functions = vec![entry::lower_program_function(program)?];
    let root_functions = collect_root_functions(&root.ast.items);
    lower_reachable_functions(&mut functions, &root_functions)?;

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
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from(["program".to_string()]);
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
            Instruction::TailCall(name) => Some(name.clone()),
            _ => None,
        })
        .collect()
}
