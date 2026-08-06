//! Native backend entry points.

mod codegen;
mod frame;
mod output;

use crate::analysis::CompileUnitAnalysis;
use crate::diagnostics::Diagnostic;
use crate::ir::{lower_executable, lower_test};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use crate::target::macho::{ExecutableImage, write_arm64_macos_executable_with_data};
use codegen::generate_arm64_darwin_entry;
use output::write_executable_image;
use std::path::Path;

pub(crate) struct BuildRequest<'a> {
    pub(crate) analysis: &'a CompileUnitAnalysis,
    pub(crate) sources: &'a SourceMap,
    pub(crate) output_path: &'a Path,
    pub(crate) target: &'a str,
}

pub(crate) fn build_executable(request: BuildRequest<'_>) -> Result<(), Vec<Diagnostic>> {
    build_process(request, None)
}

pub(crate) fn build_test(
    request: BuildRequest<'_>,
    test: &crate::test_entry::TestDeclarationId,
) -> Result<(), Vec<Diagnostic>> {
    build_process(request, Some(test))
}

fn build_process(
    request: BuildRequest<'_>,
    test: Option<&crate::test_entry::TestDeclarationId>,
) -> Result<(), Vec<Diagnostic>> {
    let BuildRequest {
        analysis,
        sources,
        output_path,
        target,
    } = request;

    if target != DEFAULT_TARGET {
        return Err(vec![Diagnostic::error(
            "E9000",
            format!("target `{target}` is not supported by the native backend yet"),
        )]);
    }

    let ir = match test {
        Some(test) => lower_test(analysis, sources, test)?,
        None => lower_executable(analysis, sources)?,
    };
    let machine_code = generate_arm64_darwin_entry(&ir)?;
    let executable_image: ExecutableImage =
        write_arm64_macos_executable_with_data(&machine_code.text, &machine_code.read_only_data);
    write_executable_image(output_path, &executable_image)
}
