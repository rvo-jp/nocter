//! Native backend entry points.

mod codegen;

use crate::analysis::CompileUnitAnalysis;
use crate::diagnostics::Diagnostic;
use crate::ir::lower_program;
use crate::target::macho::{ExecutableImage, write_arm64_macos_executable};
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
    let machine_code = generate_arm64(&ir)?;
    let executable_image: ExecutableImage = write_arm64_macos_executable(&machine_code.text);
    let _planned_output_path = output_path;
    let _planned_file_size = executable_image.bytes.len();

    Err(vec![Diagnostic::error(
        "E9000",
        format!("direct executable generation for `{target}` is not implemented yet"),
    )])
}
