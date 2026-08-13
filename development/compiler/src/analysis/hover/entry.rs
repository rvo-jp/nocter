use super::*;

pub(crate) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    if let Some(hover) = module_path_hover_for_ast(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(hover) = syntax_site_hover_for_file_analysis(file, offset) {
        return Some(hover);
    }

    if let Some(target) = file.syntax.editor_target_at(offset)
        && let crate::analysis::editor_targets::EditorTargetKind::ImportBinding(symbol) =
            &target.kind
    {
        let symbol = file.resolved.symbols.get(*symbol)?;
        let (label, documentation) =
            resolved_symbol_hover_contents(sources, analysis, &file.resolved, symbol)
                .unwrap_or_else(|| {
                    (
                        crate::analysis::presentation::symbol_presentation_without_resolution(
                            symbol,
                        ),
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

    if let Some(conversion) =
        crate::analysis::conversions::conversion_editor_info_at_offset(file, offset)
    {
        return Some(HoverInfo {
            span: conversion.focus_span,
            label: conversion.label,
            documentation: Some(conversion.documentation),
        });
    }

    if let Some(hover) =
        literal_declaration_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(hover) =
        semantic_declaration_hover_for_file_analysis(sources, analysis, file, offset)
    {
        return Some(hover);
    }

    if let Some(mut hover) =
        local_occurrence_hover_for_file_analysis(sources, analysis, file, offset)
    {
        hover.documentation = combine_documentation(
            hover.documentation,
            index_projection_markdown(file, hover.span),
        );
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

    if let Some(interpolation) =
        crate::analysis::interpolation::interpolation_editor_info_at_offset(analysis, file, offset)
    {
        return Some(HoverInfo {
            span: interpolation.focus_span,
            label: interpolation.label,
            documentation: Some(interpolation.documentation),
        });
    }

    None
}

fn index_projection_markdown(file: &FileAnalysis, focus: ByteSpan) -> Option<String> {
    let plan = file
        .typed_hir
        .index_plans()
        .find(|plan| plan.object_span == focus)?;
    let source = crate::typecheck::type_expr_presentation_label(&plan.target_ty, &file.resolved);
    let projected = plan
        .conversion
        .as_ref()
        .map(|conversion| {
            crate::typecheck::type_expr_presentation_label(&conversion.target_ty, &file.resolved)
        })
        .unwrap_or_else(|| source.clone());
    let element = crate::typecheck::type_expr_presentation_label(&plan.element_ty, &file.resolved);
    let access = match plan.access {
        crate::typecheck::TypecheckIndexAccess::Readonly => "readonly",
        crate::typecheck::TypecheckIndexAccess::Readwrite => "readwrite",
    };
    Some(format!(
        "**Index projection:** `{source}` → `{projected}`\n\n**Element:** `{element}`\n\n**Access:** {access}"
    ))
}

pub(crate) fn hover_for_text(text: &str, offset: usize) -> Option<HoverInfo> {
    let (sources, analysis) =
        crate::analysis::single_file::analyze_single_file_text("hover.nct", text)?;
    let file = analysis.root_file()?;
    hover_for_file_analysis(&sources, &analysis, file, offset)
}
