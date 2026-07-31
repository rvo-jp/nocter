//! Whole-compile-unit semantic analysis.

pub(crate) mod call_specializations;
pub(crate) mod completion;
mod completion_recovery;
pub(crate) mod definition;
pub(crate) mod hover;
pub(crate) mod references;
mod scoped_imports;
pub(crate) mod semantic;
mod single_file;
pub(crate) mod symbols;
#[cfg(test)]
pub(crate) mod test_support;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportSourceMap, PreludeSourceMap, ResolveOutput, resolve_compile_unit};
use crate::source::SourceMap;
use crate::typecheck::{
    TypecheckFacts, TypecheckSource, check_module_with_summary_sources, check_with_summary_sources,
    collect_typecheck_facts,
};
use std::cmp::Ordering;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct CompileUnit {
    root_ast: AstFile,
    files: Vec<AstFile>,
    import_sources: ImportSourceMap,
    prelude_sources: PreludeSourceMap,
    nocter_home: Option<PathBuf>,
}

impl CompileUnit {
    pub(crate) fn new(
        root_ast: AstFile,
        files: Vec<AstFile>,
        import_sources: ImportSourceMap,
        prelude_sources: PreludeSourceMap,
        nocter_home: Option<PathBuf>,
    ) -> Self {
        Self {
            root_ast,
            files,
            import_sources,
            prelude_sources,
            nocter_home,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompileUnitAnalysis {
    pub(crate) files: Vec<FileAnalysis>,
    pub(crate) import_sources: ImportSourceMap,
    pub(crate) nocter_home: Option<PathBuf>,
}

impl CompileUnitAnalysis {
    pub(crate) fn root_file(&self) -> Option<&FileAnalysis> {
        self.files.iter().find(|file| file.is_root)
    }

    pub(crate) fn file_by_source(&self, source: crate::source::SourceId) -> Option<&FileAnalysis> {
        self.files
            .iter()
            .find(|file| file.ast.span.source == source)
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
    pub(crate) typecheck_facts: TypecheckFacts,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) is_root: bool,
}

pub(crate) fn analyze_executable_compile_unit(
    sources: &SourceMap,
    unit: &CompileUnit,
) -> CompileUnitAnalysis {
    analyze_compile_unit_with_root_policy(sources, unit, RootPolicy::ExecutableEntry)
}

pub(crate) fn analyze_module_compile_unit(
    sources: &SourceMap,
    unit: &CompileUnit,
) -> CompileUnitAnalysis {
    analyze_compile_unit_with_root_policy(sources, unit, RootPolicy::ModuleOnly)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootPolicy {
    ExecutableEntry,
    ModuleOnly,
}

fn analyze_compile_unit_with_root_policy(
    sources: &SourceMap,
    unit: &CompileUnit,
    root_policy: RootPolicy,
) -> CompileUnitAnalysis {
    let root_source = unit.root_ast.span.source;
    let resolved_files = unit
        .files
        .iter()
        .map(|file| {
            resolve_compile_unit(
                sources,
                file,
                &unit.files,
                &unit.import_sources,
                &unit.prelude_sources,
            )
        })
        .collect::<Vec<_>>();
    let typecheck_sources = unit
        .files
        .iter()
        .zip(resolved_files.iter())
        .map(|(file, resolved)| TypecheckSource::new(file, resolved))
        .collect::<Vec<_>>();
    let files = unit
        .files
        .iter()
        .zip(resolved_files.iter())
        .map(|(file, resolved)| {
            let is_root = file.span.source == root_source;
            let mut diagnostics = resolved.diagnostics.clone();
            if is_root && root_policy == RootPolicy::ExecutableEntry {
                diagnostics.extend(check_with_summary_sources(
                    sources,
                    file,
                    resolved,
                    &typecheck_sources,
                ));
            } else {
                diagnostics.extend(check_module_with_summary_sources(
                    sources,
                    file,
                    resolved,
                    &typecheck_sources,
                ));
            }
            let typecheck_facts = collect_typecheck_facts(file, resolved);

            FileAnalysis {
                ast: file.clone(),
                resolved: resolved.clone(),
                typecheck_facts,
                diagnostics,
                is_root,
            }
        })
        .collect();

    CompileUnitAnalysis {
        files,
        import_sources: unit.import_sources.clone(),
        nocter_home: unit.nocter_home.clone(),
    }
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

#[cfg(test)]
mod tests {
    use super::test_support::analyze_namespace_import_text;

    #[test]
    fn return_checks_use_imported_fallible_summary_for_propagation() {
        let root_text = r#"use lib/math

func run(): i32! {
    let value = 1
    return math.okay(&value)?
}
"#;
        let module_text = r#"pub func okay(value: &i32): i32! {
    return 1
}
"#;

        let (_, analysis) = analyze_namespace_import_text(root_text, module_text);
        let diagnostics = analysis.diagnostics();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn return_checks_use_imported_fallible_summary_for_force_unwrap() {
        let root_text = r#"use lib/math

func passthrough(success: &i32, choose: bool): &i32 {
    let value = 1
    return math.maybe_fail(success, &value, choose)!
}
"#;
        let module_text = r#"primitive make_error(label: &str, value: &i32): error

pub func maybe_fail(success: &i32, failure: &i32, choose: bool): &i32! {
    if choose {
        return success
    }
    return make_error("code", failure)
}
"#;

        let (_, analysis) = analyze_namespace_import_text(root_text, module_text);
        let diagnostics = analysis.diagnostics();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
