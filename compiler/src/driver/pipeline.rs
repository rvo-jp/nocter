use crate::analysis::analyze_compile_unit;
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::path::Path;

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

pub(super) fn check_file(file: &Path) -> CheckOutput {
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
            let diagnostics = check_source(&mut sources, source);

            CheckOutput {
                root,
                root_absolute_path,
                diagnostics,
            }
        }
        Err(diagnostic) => CheckOutput {
            root: file.to_string_lossy().into_owned(),
            root_absolute_path: canonical_absolute_string(file),
            diagnostics: vec![diagnostic],
        },
    }
}

pub(super) fn check_source(sources: &mut SourceMap, source: SourceId) -> Vec<Diagnostic> {
    check_source_with_options(sources, source, &FrontendOptions::default())
}

pub(super) fn check_source_with_options(
    sources: &mut SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> Vec<Diagnostic> {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return diagnostics,
    };

    analyze_compile_unit(sources, &unit).diagnostics()
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}
