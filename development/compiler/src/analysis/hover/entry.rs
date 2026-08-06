use super::*;

pub(crate) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    if let Some(hover) = module_path_hover_for_ast(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(target) = crate::analysis::editor_targets::editor_target_at_offset(file, offset)
        && let crate::analysis::editor_targets::EditorTargetKind::ImportBinding(symbol) =
            target.kind
    {
        let (label, documentation) = resolved_symbol_hover_contents(sources, analysis, symbol)
            .unwrap_or_else(|| {
                (
                    crate::analysis::presentation::symbol_presentation_without_resolution(symbol),
                    None,
                )
            });
        return Some(HoverInfo {
            span: target.focus_span,
            label,
            documentation,
        });
    }

    if let Some(literal) = crate::analysis::literals::literal_editor_info_at_offset(
        analysis,
        file,
        offset,
        crate::analysis::literals::LiteralCursorRegion::Hover,
    ) {
        return Some(HoverInfo {
            span: literal.focus_span,
            label: literal.label,
            documentation: combine_documentation(
                target_documentation(sources, analysis, literal.declaration_shape_span),
                semantic_documentation(sources, analysis, literal.declaration_span),
            ),
        });
    }

    if let Some((span, documentation)) =
        crate::analysis::iteration::sequence_spread_operator_hover(analysis, file, offset)
    {
        return Some(HoverInfo {
            span,
            label: "...".to_string(),
            documentation: Some(documentation),
        });
    }

    if let Some(hover) =
        literal_declaration_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(hover) = local_occurrence_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(hover) = type_occurrence_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(hover) = call_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(hover) =
        property_occurrence_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(hover) =
        callable_symbol_occurrence_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(hover) =
        callable_member_occurrence_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.target.focus_span, offset))
    {
        let attached = documentation
            .get(symbol.target.focus_span.start)
            .map(str::to_string);
        let semantic = semantic_documentation(sources, analysis, symbol.target.declaration_span);
        let region =
            crate::analysis::regions::region_markdown(sources, file, symbol.target.focus_span);
        let iteration =
            crate::analysis::iteration::iteration_markdown_at_offset(analysis, file, offset);
        return Some(HoverInfo {
            span: symbol.target.focus_span,
            label: symbol.label.clone(),
            documentation: combine_documentation(
                combine_documentation(combine_documentation(attached, semantic), region),
                iteration,
            ),
        });
    }

    if let Some(interpolation) =
        crate::analysis::interpolation::interpolation_editor_info_at_offset(file, offset)
    {
        return Some(HoverInfo {
            span: interpolation.focus_span,
            label: interpolation.label,
            documentation: Some(interpolation.documentation),
        });
    }

    resolved_reference_at_offset(&file.resolved, offset).map(|(span, reference)| {
        let (label, documentation) =
            resolved_reference_hover_contents(sources, analysis, &reference);
        HoverInfo {
            span,
            label,
            documentation: combine_documentation(
                documentation,
                crate::analysis::iteration::iteration_markdown_at_offset(analysis, file, offset),
            ),
        }
    })
}

pub(crate) fn hover_for_text(text: &str, offset: usize) -> Option<HoverInfo> {
    let (sources, analysis) =
        crate::analysis::single_file::analyze_single_file_text("hover.nct", text)?;
    let file = analysis.root_file()?;
    hover_for_file_analysis(&sources, &analysis, file, offset)
}

pub(crate) fn definition_target_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<crate::analysis::editor_targets::SourceTarget> {
    let symbols = hover_symbols_for_ast(text, ast);
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.target.focus_span, offset))
    {
        return Some(symbol.target);
    }

    resolved_reference_at_offset(resolved, offset).map(|(origin, reference)| {
        crate::analysis::editor_targets::SourceTarget::new(origin, reference.declaration_span())
    })
}
