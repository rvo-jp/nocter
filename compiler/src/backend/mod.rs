//! Native backend entry points.

mod codegen;

use crate::analysis::CompileUnitAnalysis;
use crate::diagnostics::Diagnostic;
use crate::ir::lower_program;
use codegen::generate_arm64;
use std::path::Path;

pub(crate) struct BuildRequest<'a> {
    pub(crate) analysis: &'a CompileUnitAnalysis,
    pub(crate) output_path: &'a Path,
    pub(crate) target: &'a str,
}

pub(crate) fn build_executable(request: BuildRequest<'_>) -> Result<(), Vec<Diagnostic>> {
    let BuildRequest {
        analysis,
        output_path,
        target,
    } = request;

    let ir = lower_program(analysis)?;
    let _machine_code = generate_arm64(&ir)?;
    let _planned_output_path = output_path;

    Err(vec![Diagnostic::error(
        "E9000",
        format!("direct executable generation for `{target}` is not implemented yet"),
    )])
}
