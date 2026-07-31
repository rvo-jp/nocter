use super::buildability::v0_buildability_diagnostics;
use crate::analysis::analyze_executable_compile_unit;
use crate::backend::{BuildRequest, build_executable};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::path::{Path, PathBuf};

struct FrontendOutput {
    root: String,
    root_absolute_path: Option<String>,
    sources: SourceMap,
    analysis: Option<crate::analysis::CompileUnitAnalysis>,
    diagnostics: Vec<Diagnostic>,
}

pub(super) struct CheckOutput {
    pub root: String,
    pub root_absolute_path: Option<String>,
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) struct BuildOutput {
    pub output_path: PathBuf,
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) fn check_file_with_target(file: &Path, target: &str) -> CheckOutput {
    let options = frontend_options_for_target(target);
    let output = analyze_file(file, &options);

    CheckOutput {
        root: output.root,
        root_absolute_path: output.root_absolute_path,
        sources: output.sources,
        diagnostics: output.diagnostics,
    }
}

pub(super) fn build_file_with_target(file: &Path, target: &str) -> BuildOutput {
    let output_path = default_executable_path(file);
    build_file_to_path_with_target(file, &output_path, target)
}

pub(super) fn build_file_to_path_with_target(
    file: &Path,
    output_path: &Path,
    target: &str,
) -> BuildOutput {
    let options = frontend_options_for_target(target);
    build_file_to_path_with_options(file, output_path, &options)
}

fn build_file_to_path_with_options(
    file: &Path,
    output_path: &Path,
    options: &FrontendOptions,
) -> BuildOutput {
    let output = analyze_file(file, options);

    if !output.diagnostics.is_empty() {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics: output.diagnostics,
        };
    }

    let Some(analysis) = output.analysis.as_ref() else {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics: vec![Diagnostic::error(
                "E0201",
                "frontend analysis completed without diagnostics but produced no analysis output",
            )],
        };
    };

    let diagnostics = v0_buildability_diagnostics(&output.sources, analysis);
    if !diagnostics.is_empty() {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics,
        };
    }

    let diagnostics = match build_executable(BuildRequest {
        analysis,
        sources: &output.sources,
        output_path,
        target: options.target.as_str(),
    }) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };

    BuildOutput {
        output_path: output_path.to_path_buf(),
        sources: output.sources,
        diagnostics,
    }
}

fn frontend_options_for_target(target: &str) -> FrontendOptions {
    FrontendOptions {
        target: target.to_string(),
        ..FrontendOptions::default()
    }
}

fn analyze_file(file: &Path, options: &FrontendOptions) -> FrontendOutput {
    let mut sources = SourceMap::new();

    match sources.load_file(file) {
        Ok(source) => {
            let source_file = sources
                .get(source)
                .expect("loaded source id must resolve in source map");
            let root = source_file.display_path().to_string();
            let root_absolute_path = source_file
                .absolute_path()
                .map(|path| path.to_string_lossy().into_owned());
            let (analysis, diagnostics) = analyze_source(&mut sources, source, options);

            FrontendOutput {
                root,
                root_absolute_path,
                sources,
                analysis,
                diagnostics,
            }
        }
        Err(diagnostic) => FrontendOutput {
            root: file.to_string_lossy().into_owned(),
            root_absolute_path: canonical_absolute_string(file),
            sources,
            analysis: None,
            diagnostics: vec![diagnostic],
        },
    }
}

fn analyze_source(
    sources: &mut SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> (
    Option<crate::analysis::CompileUnitAnalysis>,
    Vec<Diagnostic>,
) {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return (None, diagnostics),
    };

    let analysis = analyze_executable_compile_unit(sources, &unit);
    let diagnostics = analysis.diagnostics();

    (Some(analysis), diagnostics)
}

fn default_executable_path(source_path: &Path) -> PathBuf {
    match source_path.file_stem() {
        Some(stem) => source_path.with_file_name(stem),
        None => PathBuf::from("a.out"),
    }
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
