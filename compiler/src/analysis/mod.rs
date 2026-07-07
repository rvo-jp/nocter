//! Whole-compile-unit semantic analysis.

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportSourceMap, ResolveOutput, resolve_compile_unit};
use crate::source::SourceMap;
use crate::typecheck::{check, check_module};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub(crate) struct CompileUnit {
    root_ast: AstFile,
    files: Vec<AstFile>,
    import_sources: ImportSourceMap,
}

impl CompileUnit {
    pub(crate) fn new(
        root_ast: AstFile,
        files: Vec<AstFile>,
        import_sources: ImportSourceMap,
    ) -> Self {
        Self {
            root_ast,
            files,
            import_sources,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompileUnitAnalysis {
    pub(crate) files: Vec<FileAnalysis>,
}

impl CompileUnitAnalysis {
    pub(crate) fn root_file(&self) -> Option<&FileAnalysis> {
        self.files.iter().find(|file| file.is_root)
    }

    pub(crate) fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = self
            .files
            .iter()
            .enumerate()
            .flat_map(|(file_index, file)| {
                file.diagnostics
                    .iter()
                    .cloned()
                    .map(move |diagnostic| (file_index, diagnostic))
            })
            .collect::<Vec<_>>();

        diagnostics.sort_by(|(left_file, left), (right_file, right)| {
            compare_diagnostics(*left_file, left, *right_file, right)
        });

        diagnostics
            .into_iter()
            .map(|(_, diagnostic)| diagnostic)
            .collect()
    }
}

// The CLI currently flattens diagnostics, while editor tooling will consume the
// retained AST and resolver state for a specific file.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct FileAnalysis {
    pub(crate) ast: AstFile,
    pub(crate) resolved: ResolveOutput,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) is_root: bool,
}

pub(crate) fn analyze_compile_unit_with_entry(
    sources: &SourceMap,
    unit: &CompileUnit,
    entry_name: &str,
) -> CompileUnitAnalysis {
    let root_source = unit.root_ast.span.source;
    let files = unit
        .files
        .iter()
        .map(|file| {
            let is_root = file.span.source == root_source;
            let resolved = resolve_compile_unit(sources, file, &unit.files, &unit.import_sources);
            let mut diagnostics = resolved.diagnostics.clone();
            if is_root {
                diagnostics.extend(check(sources, file, &resolved, entry_name));
            } else {
                diagnostics.extend(check_module(sources, file, &resolved));
            }

            FileAnalysis {
                ast: file.clone(),
                resolved,
                diagnostics,
                is_root,
            }
        })
        .collect();

    CompileUnitAnalysis { files }
}

fn compare_diagnostics(
    left_file: usize,
    left: &Diagnostic,
    right_file: usize,
    right: &Diagnostic,
) -> Ordering {
    left_file
        .cmp(&right_file)
        .then_with(|| compare_diagnostic_primary_spans(left, right))
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.message.cmp(&right.message))
}

fn compare_diagnostic_primary_spans(left: &Diagnostic, right: &Diagnostic) -> Ordering {
    match (left.primary_span.as_deref(), right.primary_span.as_deref()) {
        (Some(left), Some(right)) => left
            .start_byte
            .cmp(&right.start_byte)
            .then_with(|| left.end_byte.cmp(&right.end_byte)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
