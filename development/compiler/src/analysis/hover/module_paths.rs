use super::*;
pub(in crate::analysis::hover) fn module_path_hover_for_ast(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let path = file.syntax.module_path_at(offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;
    let imported_source = sources.get(imported_file.ast.span.source)?;
    let docs = attach_documentation(imported_file.ast.span.source, imported_source.text(), &[]);

    Some(HoverInfo {
        span: path.span,
        label: format!("module {}", path.value),
        documentation: docs.file().map(str::to_string),
    })
}
