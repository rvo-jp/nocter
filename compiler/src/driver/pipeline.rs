use crate::analysis::analyze_compile_unit;
use crate::backend::{BuildRequest, build_executable};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use std::path::{Path, PathBuf};

struct FrontendOutput {
    root: String,
    root_absolute_path: Option<String>,
    analysis: Option<crate::analysis::CompileUnitAnalysis>,
    diagnostics: Vec<Diagnostic>,
}

pub(super) struct CheckOutput {
    pub root: String,
    pub root_absolute_path: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) struct BuildOutput {
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) fn check_file(file: &Path) -> CheckOutput {
    let output = analyze_file(file);

    CheckOutput {
        root: output.root,
        root_absolute_path: output.root_absolute_path,
        diagnostics: output.diagnostics,
    }
}

pub(super) fn build_file(file: &Path) -> BuildOutput {
    let output_path = default_executable_path(file);
    let output = analyze_file(file);

    if !output.diagnostics.is_empty() {
        return BuildOutput {
            diagnostics: output.diagnostics,
        };
    }

    let Some(analysis) = output.analysis.as_ref() else {
        return BuildOutput {
            diagnostics: vec![Diagnostic::error(
                "E0201",
                "frontend analysis completed without diagnostics but produced no analysis output",
            )],
        };
    };

    let diagnostics = match build_executable(BuildRequest {
        analysis,
        output_path: &output_path,
        target: DEFAULT_TARGET,
    }) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };

    BuildOutput { diagnostics }
}

fn analyze_file(file: &Path) -> FrontendOutput {
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
            let (analysis, diagnostics) = analyze_source(&mut sources, source);

            FrontendOutput {
                root,
                root_absolute_path,
                analysis,
                diagnostics,
            }
        }
        Err(diagnostic) => FrontendOutput {
            root: file.to_string_lossy().into_owned(),
            root_absolute_path: canonical_absolute_string(file),
            analysis: None,
            diagnostics: vec![diagnostic],
        },
    }
}

fn analyze_source(
    sources: &mut SourceMap,
    source: SourceId,
) -> (
    Option<crate::analysis::CompileUnitAnalysis>,
    Vec<Diagnostic>,
) {
    let unit = match load_compile_unit(sources, source, &FrontendOptions::default()) {
        Ok(unit) => unit,
        Err(diagnostics) => return (None, diagnostics),
    };

    let analysis = analyze_compile_unit(sources, &unit);
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn derives_default_executable_path_from_source_path() {
        assert_eq!(
            default_executable_path(Path::new("src/app.nct")),
            PathBuf::from("src/app")
        );
    }

    #[test]
    fn derives_fallback_executable_path_for_empty_source_path() {
        assert_eq!(
            default_executable_path(Path::new("")),
            PathBuf::from("a.out")
        );
    }
}
