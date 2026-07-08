//! Front-end source loading, parsing, and compile-unit construction.

mod diagnostics;
mod imports;
mod parsing;
mod prelude;

#[cfg(test)]
mod tests;

use crate::analysis::CompileUnit;
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportSource, ImportSourceMap};
use crate::source::{SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use diagnostics::import_source_diagnostic;
use imports::{import_access_for_source, import_paths, resolve_import_path};
use parsing::parse_source_for_check;
use prelude::{should_synthesize_prelude, synthesize_prelude_use};

#[derive(Debug, Clone)]
pub(crate) struct FrontendOptions {
    pub(crate) nocter_home: Option<PathBuf>,
    pub(crate) target: String,
}

impl Default for FrontendOptions {
    fn default() -> Self {
        Self {
            nocter_home: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }
}

pub(crate) fn load_compile_unit(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> Result<CompileUnit, Vec<Diagnostic>> {
    let mut queue = VecDeque::from([root]);
    let mut queued_sources = HashSet::from([root]);
    let mut loaded_sources_by_path = std::collections::HashMap::new();
    let mut import_sources = ImportSourceMap::new();
    let mut resolved_nocter_home = None;
    let mut diagnostics = Vec::new();
    let mut root_ast = None;
    let mut files = Vec::new();

    for (path, source) in sources.sources_with_absolute_paths() {
        loaded_sources_by_path.insert(path.to_path_buf(), source);
    }

    while let Some(source) = queue.pop_front() {
        let mut ast = match parse_source_for_check(sources, source) {
            Ok(ast) => ast,
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics);
                continue;
            }
        };

        if should_synthesize_prelude(sources, source, &ast, options, &mut resolved_nocter_home) {
            synthesize_prelude_use(source, &mut ast);
        }

        if source == root {
            root_ast = Some(ast.clone());
        }
        files.push(ast.clone());

        for path in import_paths(&ast) {
            let canonical = match resolve_import_path(
                sources,
                source,
                path,
                options,
                &mut resolved_nocter_home,
            ) {
                Ok(path) => path,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let imported = match loaded_sources_by_path.get(&canonical).copied() {
                Some(source) => source,
                None => match sources.load_file(&canonical) {
                    Ok(source) => {
                        loaded_sources_by_path.insert(canonical, source);
                        source
                    }
                    Err(error) => {
                        diagnostics.push(import_source_diagnostic(
                            sources,
                            path.span,
                            &path.value,
                            error,
                        ));
                        continue;
                    }
                },
            };

            import_sources.insert(
                path.span,
                ImportSource {
                    source: imported,
                    access: import_access_for_source(
                        sources,
                        source,
                        options,
                        &resolved_nocter_home,
                    ),
                },
            );

            if queued_sources.insert(imported) {
                queue.push_back(imported);
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let Some(root_ast) = root_ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "root source did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(CompileUnit::new(root_ast, files, import_sources))
}
