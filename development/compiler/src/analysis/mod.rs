//! Whole-compile-unit semantic analysis.

mod call_sites;
pub(crate) mod call_specializations;
pub(crate) mod coercions;
mod collection_for_recovery;
pub(crate) mod completion;
mod completion_recovery;
pub(crate) mod constructions;
pub(crate) mod conversions;
pub(crate) mod definition;
mod delimiter_recovery;
mod drop_dependencies;
pub(crate) mod editor_targets;
mod expected_completion;
pub(crate) mod hover;
pub(crate) mod implementation;
mod import_completion;
pub(crate) mod inlay_hints;
pub(crate) mod interpolation;
mod invalidation;
pub(crate) mod iteration;
mod literal_recovery;
pub(crate) mod literal_specializations;
pub(crate) mod literals;
pub(crate) mod occurrences;
mod opaque_results;
pub(crate) mod package_index;
pub(crate) mod presentation;
pub(crate) mod references;
mod region_recovery;
pub(crate) mod regions;
mod scoped_imports;
pub(crate) mod semantic;
pub(crate) mod signature_help;
mod single_file;
pub(crate) mod source_edits;
pub(crate) mod symbols;
#[cfg(test)]
pub(crate) mod test_support;

pub(crate) use collection_for_recovery::collection_for_recovery_text;
pub(crate) use completion_recovery::{completion_recovery_overlay, signature_recovery_texts};
pub(crate) use delimiter_recovery::block_recovery_text;
pub(crate) use interpolation::{
    interpolation_completion_recovery_overlay, interpolation_recovery_text,
    interpolation_signature_recovery_texts,
};
pub(crate) use literal_recovery::{literal_recovery_overlay, literal_recovery_text};
pub(crate) use region_recovery::region_recovery_text;
mod visible_locals;

use crate::ast::AstFile;
use crate::callable_bodies::CallableBodyIndex;
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportSourceMap, PreludeSourceMap, ResolveOutput};
use crate::semantic::SemanticDb;
use crate::semantics::TrustedDeclarationInputs;
use crate::source::SourceMap;
use crate::source_scopes::SourceScopeMap;
use crate::typecheck::{
    TypecheckCompileUnitContext, TypecheckFacts, TypecheckSource,
    check_module_with_compile_unit_context, check_with_compile_unit_context,
    collect_typecheck_facts,
};
use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct CompileUnit {
    root_ast: AstFile,
    files: Vec<AstFile>,
    semantic_db: Arc<SemanticDb>,
    import_sources: ImportSourceMap,
    prelude_sources: PreludeSourceMap,
    nocter_home: Option<PathBuf>,
    trusted_declarations: TrustedDeclarationInputs,
    callable_bodies: CallableBodyIndex,
    source_scopes: SourceScopeMap,
}

impl CompileUnit {
    pub(crate) fn new(
        root_ast: AstFile,
        files: Vec<AstFile>,
        import_sources: ImportSourceMap,
        prelude_sources: PreludeSourceMap,
        nocter_home: Option<PathBuf>,
    ) -> Self {
        let semantic_db = Arc::new(SemanticDb::from_files(&files));
        Self::new_with_semantic_db(
            root_ast,
            files,
            import_sources,
            prelude_sources,
            nocter_home,
            semantic_db,
        )
    }

    pub(crate) fn new_with_semantic_db(
        root_ast: AstFile,
        files: Vec<AstFile>,
        import_sources: ImportSourceMap,
        prelude_sources: PreludeSourceMap,
        nocter_home: Option<PathBuf>,
        semantic_db: Arc<SemanticDb>,
    ) -> Self {
        Self {
            root_ast,
            files,
            semantic_db,
            import_sources,
            prelude_sources,
            nocter_home,
            trusted_declarations: TrustedDeclarationInputs::default(),
            callable_bodies: CallableBodyIndex::default(),
            source_scopes: SourceScopeMap::default(),
        }
    }

    pub(crate) fn with_trusted_declarations(
        mut self,
        trusted_declarations: TrustedDeclarationInputs,
    ) -> Self {
        self.trusted_declarations = trusted_declarations;
        self
    }

    pub(crate) fn with_callable_bodies(mut self, callable_bodies: CallableBodyIndex) -> Self {
        assert!(
            Arc::ptr_eq(&self.semantic_db, &callable_bodies.semantic_db()),
            "compile unit and callable-body index must share one semantic database"
        );
        self.callable_bodies = callable_bodies;
        self
    }

    pub(crate) fn with_source_scopes(mut self, source_scopes: SourceScopeMap) -> Self {
        self.source_scopes = source_scopes;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompileUnitAnalysis {
    pub(crate) semantic_db: Arc<SemanticDb>,
    pub(crate) files: Vec<FileAnalysis>,
    pub(crate) import_sources: ImportSourceMap,
    pub(crate) nocter_home: Option<PathBuf>,
    pub(crate) callable_bodies: CallableBodyIndex,
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
    pub(crate) occurrences: occurrences::SemanticOccurrenceIndex,
    pub(crate) callable_declarations: presentation::CallableDeclarationIndex,
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
    let mut analyzed_files = unit.files.clone();
    let initial_resolution_context = crate::timing::measure("analysis.index_modules", || {
        crate::resolve::ResolveCompileUnitContext::with_semantic_db(
            &analyzed_files,
            &unit.import_sources,
            unit.semantic_db.clone(),
        )
    });
    let trusted_declarations = unit
        .trusted_declarations
        .bind(initial_resolution_context.semantic_db());
    let initial_resolved_files = crate::timing::measure("analysis.resolve_initial", || {
        analyzed_files
            .iter()
            .map(|file| {
                let mut resolved = crate::resolve::resolve_compile_unit_with_context(
                    sources,
                    file,
                    &unit.files,
                    &unit.import_sources,
                    &unit.prelude_sources,
                    &unit.callable_bodies,
                    &unit.source_scopes,
                    &initial_resolution_context,
                );
                resolved.trusted_declarations = trusted_declarations.clone();
                resolved.diagnostics.retain(|diagnostic| {
                    diagnostic_belongs_to_file(sources, diagnostic, file.span.source)
                        || (diagnostic.primary_span.is_none() && file.span.source == root_source)
                });
                resolved
            })
            .collect::<Vec<_>>()
    });
    let (opaque_diagnostics, opaque_changed_files) =
        crate::timing::measure("analysis.elaborate_opaque", || {
            let mut diagnostics = Vec::new();
            let mut changed_files = Vec::with_capacity(analyzed_files.len());
            for (file, resolved) in analyzed_files.iter_mut().zip(initial_resolved_files.iter()) {
                let facts = collect_typecheck_facts(file, resolved);
                let (file_diagnostics, file_changed) =
                    opaque_results::elaborate_file(sources, file, resolved, &facts);
                diagnostics.extend(file_diagnostics);
                changed_files.push(file_changed);
            }
            (diagnostics, changed_files)
        });
    let opaque_changed_sources = analyzed_files
        .iter()
        .zip(opaque_changed_files.iter())
        .filter_map(|(file, changed)| changed.then_some(file.span.source))
        .collect::<std::collections::HashSet<_>>();
    let opaque_affected_sources = invalidation::reverse_dependency_closure(
        &opaque_changed_sources,
        &unit.import_sources,
        &unit.prelude_sources,
    );
    let resolved_files = if !opaque_affected_sources.is_empty() {
        let elaborated_resolution_context =
            crate::timing::measure("analysis.index_opaque_modules", || {
                crate::resolve::ResolveCompileUnitContext::with_semantic_db(
                    &analyzed_files,
                    &unit.import_sources,
                    initial_resolution_context.semantic_db(),
                )
            });
        crate::timing::measure("analysis.resolve_opaque_dependents", || {
            initial_resolved_files
                .into_iter()
                .enumerate()
                .map(|(index, initial)| {
                    if !opaque_affected_sources.contains(&analyzed_files[index].span.source) {
                        return initial;
                    }
                    let file = &analyzed_files[index];
                    let mut resolved = crate::resolve::resolve_compile_unit_with_context(
                        sources,
                        file,
                        &analyzed_files,
                        &unit.import_sources,
                        &unit.prelude_sources,
                        &unit.callable_bodies,
                        &unit.source_scopes,
                        &elaborated_resolution_context,
                    );
                    resolved.trusted_declarations = trusted_declarations.clone();
                    resolved.diagnostics.retain(|diagnostic| {
                        diagnostic_belongs_to_file(sources, diagnostic, file.span.source)
                            || (diagnostic.primary_span.is_none()
                                && file.span.source == root_source)
                    });
                    resolved
                })
                .collect::<Vec<_>>()
        })
    } else {
        initial_resolved_files
    };
    let typecheck_sources = analyzed_files
        .iter()
        .zip(resolved_files.iter())
        .map(|(file, resolved)| TypecheckSource::new(file, resolved))
        .collect::<Vec<_>>();
    let typecheck_context = crate::timing::measure("analysis.typecheck_context", || {
        TypecheckCompileUnitContext::new(&typecheck_sources)
    });
    let files = crate::timing::measure("analysis.typecheck_and_indexes", || {
        analyzed_files
            .iter()
            .zip(resolved_files.iter())
            .map(|(file, resolved)| {
                let is_root = file.span.source == root_source;
                let mut diagnostics = resolved.diagnostics.clone();
                diagnostics.extend(
                    opaque_diagnostics
                        .iter()
                        .filter(|diagnostic| {
                            diagnostic_belongs_to_file(sources, diagnostic, file.span.source)
                        })
                        .cloned(),
                );
                if is_root && root_policy == RootPolicy::ExecutableEntry {
                    diagnostics.extend(check_with_compile_unit_context(
                        sources,
                        file,
                        resolved,
                        &typecheck_context,
                    ));
                } else {
                    diagnostics.extend(check_module_with_compile_unit_context(
                        sources,
                        file,
                        resolved,
                        &typecheck_context,
                    ));
                }
                let typecheck_facts = collect_typecheck_facts(file, resolved);
                let occurrences =
                    occurrences::SemanticOccurrenceIndex::new(file, resolved, &typecheck_facts);
                let callable_declarations = presentation::CallableDeclarationIndex::new(file);

                FileAnalysis {
                    ast: file.clone(),
                    resolved: resolved.clone(),
                    typecheck_facts,
                    occurrences,
                    callable_declarations,
                    diagnostics,
                    is_root,
                }
            })
            .collect()
    });

    CompileUnitAnalysis {
        semantic_db: initial_resolution_context.semantic_db(),
        files,
        import_sources: unit.import_sources.clone(),
        nocter_home: unit.nocter_home.clone(),
        callable_bodies: unit.callable_bodies.clone(),
    }
}

fn diagnostic_belongs_to_file(
    sources: &SourceMap,
    diagnostic: &Diagnostic,
    source: crate::source::SourceId,
) -> bool {
    let Some(primary) = diagnostic.primary_span.as_deref() else {
        return false;
    };
    let Some(file) = sources.get(source) else {
        return false;
    };

    match (primary.absolute_path.as_deref(), file.absolute_path()) {
        (Some(primary), Some(path)) => primary == path.to_string_lossy(),
        _ => primary.file == file.display_path(),
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

pub func maybe_fail(success: &i32, failure: &i32, choose: bool): &i32! from success | failure {
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
