mod entry;
mod errors;
mod literals;

#[cfg(test)]
mod tests;

use super::{Function, IrModule};
use crate::analysis::CompileUnitAnalysis;
use crate::ast::Item;
use crate::diagnostics::Diagnostic;

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

    let function: Function = entry::lower_program_function(program)?;

    Ok(IrModule::new(vec![function]))
}
