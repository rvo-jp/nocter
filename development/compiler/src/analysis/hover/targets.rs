use super::*;
use crate::ast::TypeExpr;

pub(in crate::analysis::hover) fn call_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if !matches!(
        occurrence.kind,
        crate::analysis::occurrences::SemanticOccurrenceKind::Function
            | crate::analysis::occurrences::SemanticOccurrenceKind::Method
    ) {
        return None;
    }
    let signature =
        crate::analysis::signature_help::call_signature_at_offset(sources, analysis, file, offset)?;
    let target = match occurrence.identity {
        Some(crate::analysis::occurrences::SemanticIdentity::Declaration(target))
        | Some(crate::analysis::occurrences::SemanticIdentity::Member(target)) => Some(target),
        _ => None,
    };
    Some(HoverInfo {
        span: occurrence.focus_span,
        label: signature.label,
        documentation: combine_documentation(
            signature.documentation,
            target.and_then(|target| {
                semantic_documentation_for_result(
                    sources,
                    analysis,
                    target,
                    &signature.result_type,
                    &file.resolved,
                )
            }),
        ),
    })
}

pub(in crate::analysis::hover) fn property_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if occurrence.kind != crate::analysis::occurrences::SemanticOccurrenceKind::Property {
        return None;
    }
    let crate::analysis::occurrences::SemanticIdentity::Member(target) = occurrence.identity?
    else {
        return None;
    };
    for target_file in &analysis.files {
        for symbol in target_file.resolved.symbols.symbols() {
            let SymbolKind::Type(owner) = &symbol.kind else {
                continue;
            };
            let owner_label =
                crate::analysis::presentation::type_owner_presentation_label(owner, &file.resolved);
            if let Some(field) = owner.fields.iter().find(|field| field.name_span == target) {
                let ty = file
                    .typecheck_facts
                    .field_type_expr(occurrence.focus_span)
                    .unwrap_or(&field.ty);
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: field_member_label(
                        &owner_label,
                        &field.name,
                        &crate::typecheck::type_expr_presentation_label(ty, &file.resolved),
                    ),
                    documentation: target_documentation(sources, analysis, target),
                });
            }
            if let Some(variant) = owner
                .variants
                .iter()
                .find(|variant| variant.name_span == target)
            {
                let payload = variant
                    .payload
                    .iter()
                    .map(|parameter| {
                        format!(
                            "{}: {}",
                            parameter.name,
                            crate::typecheck::type_expr_presentation_label(
                                &parameter.ty,
                                &file.resolved,
                            )
                        )
                    })
                    .collect::<Vec<_>>();
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: enum_variant_member_label(&owner_label, &variant.name, &payload),
                    documentation: target_documentation(sources, analysis, target),
                });
            }
        }
        for surface in [
            crate::builtin_types::BuiltinTypeOwner::Str,
            crate::builtin_types::BuiltinTypeOwner::Slice,
        ]
        .into_iter()
        .filter_map(|owner| target_file.resolved.builtin_type_surface(owner))
        {
            if let Some(method) = surface
                .symbol
                .methods
                .iter()
                .find(|method| method.name_span == target)
            {
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: crate::analysis::presentation::method_or_operator_presentation(
                        &surface.symbol,
                        method,
                        &file.resolved,
                    ),
                    documentation: combine_documentation(
                        target_documentation(sources, analysis, target),
                        semantic_documentation(sources, analysis, target),
                    ),
                });
            }
        }
    }
    None
}

pub(in crate::analysis::hover) fn literal_declaration_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if occurrence.kind != crate::analysis::occurrences::SemanticOccurrenceKind::Literal {
        return None;
    }
    let crate::analysis::occurrences::SemanticIdentity::Member(target) = occurrence.identity?
    else {
        return None;
    };
    for target_file in &analysis.files {
        for symbol in target_file.resolved.symbols.symbols() {
            let SymbolKind::Type(owner) = &symbol.kind else {
                continue;
            };
            let Some(literal) = owner
                .literals
                .iter()
                .find(|literal| literal.shape_span == target)
            else {
                continue;
            };
            return Some(HoverInfo {
                span: occurrence.focus_span,
                label: crate::analysis::presentation::literal_signature_presentation(
                    owner,
                    literal,
                    &file.resolved,
                )
                .render(),
                documentation: combine_documentation(
                    target_documentation(sources, analysis, target),
                    semantic_documentation(sources, analysis, literal.declaration_span),
                ),
            });
        }
    }
    None
}

pub(in crate::analysis::hover) fn local_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    let crate::analysis::occurrences::SemanticIdentity::Local(target) = occurrence.identity? else {
        return None;
    };
    let target_file = analysis.file_by_source(target.source)?;
    let symbol = target_file
        .resolved
        .local_symbols()
        .find(|symbol| symbol.name_span == target)?;
    let label = crate::analysis::presentation::local_presentation(
        symbol,
        target_file.typecheck_facts.binding_type_expr(target),
        &target_file.resolved,
    )
    .render();
    Some(HoverInfo {
        span: occurrence.focus_span,
        label,
        documentation: combine_documentation(
            combine_documentation(
                target_documentation(sources, analysis, target),
                semantic_documentation(sources, analysis, target),
            ),
            combine_documentation(
                crate::analysis::regions::region_markdown(sources, target_file, target),
                crate::analysis::iteration::iteration_markdown_at_offset(analysis, file, offset),
            ),
        ),
    })
}

pub(in crate::analysis::hover) fn callable_symbol_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if occurrence.kind != crate::analysis::occurrences::SemanticOccurrenceKind::Function {
        return None;
    }
    let crate::analysis::occurrences::SemanticIdentity::Declaration(target) = occurrence.identity?
    else {
        return None;
    };
    let target_file = analysis.file_by_source(target.source)?;
    let symbol = target_file
        .resolved
        .symbols
        .symbols()
        .find(|symbol| symbol.declaration_span == target)?;
    let (kind, signature) = match &symbol.kind {
        SymbolKind::Function(signature) => ("func", signature),
        SymbolKind::Primitive(signature) => ("primitive", signature),
        SymbolKind::Type(_) | SymbolKind::Imported(_) => return None,
    };
    Some(HoverInfo {
        span: occurrence.focus_span,
        label: crate::analysis::presentation::callable_signature_presentation(
            kind,
            &symbol.name,
            signature,
            &file.resolved,
        )
        .render(),
        documentation: combine_documentation(
            target_documentation(sources, analysis, target),
            semantic_documentation(sources, analysis, target),
        ),
    })
}

pub(in crate::analysis::hover) fn callable_member_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if !matches!(
        occurrence.kind,
        crate::analysis::occurrences::SemanticOccurrenceKind::Function
            | crate::analysis::occurrences::SemanticOccurrenceKind::Method
    ) {
        return None;
    }
    let crate::analysis::occurrences::SemanticIdentity::Member(target) = occurrence.identity?
    else {
        return None;
    };
    for target_file in &analysis.files {
        for symbol in target_file.resolved.symbols.symbols() {
            let SymbolKind::Type(owner) = &symbol.kind else {
                continue;
            };
            if let Some(function) = owner
                .associated_functions
                .iter()
                .find(|function| function.name_span == target)
            {
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: crate::analysis::presentation::associated_function_presentation(
                        owner,
                        function,
                        &file.resolved,
                    )
                    .render(),
                    documentation: combine_documentation(
                        target_documentation(sources, analysis, target),
                        semantic_documentation(sources, analysis, target),
                    ),
                });
            }
            if let Some(method) = owner
                .methods
                .iter()
                .find(|method| method.name_span == target)
            {
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: crate::analysis::presentation::method_or_operator_presentation(
                        owner,
                        method,
                        &file.resolved,
                    ),
                    documentation: combine_documentation(
                        target_documentation(sources, analysis, target),
                        semantic_documentation(sources, analysis, target),
                    ),
                });
            }
            if let Some(drop_) = &owner.destructor
                && drop_.name_span == target
            {
                return Some(HoverInfo {
                    span: occurrence.focus_span,
                    label: crate::analysis::presentation::drop_presentation(
                        owner,
                        drop_,
                        &file.resolved,
                    ),
                    documentation: target_documentation(sources, analysis, target),
                });
            }
        }
    }
    None
}

pub(in crate::analysis::hover) fn semantic_documentation(
    _sources: &SourceMap,
    _analysis: &CompileUnitAnalysis,
    _target_span: ByteSpan,
) -> Option<String> {
    None
}

fn semantic_documentation_for_result(
    _sources: &SourceMap,
    _analysis: &CompileUnitAnalysis,
    _target_span: ByteSpan,
    _result_type: &crate::ast::TypeExpr,
    _resolved: &crate::resolve::ResolveOutput,
) -> Option<String> {
    None
}

pub(in crate::analysis::hover) fn combine_documentation(
    first: Option<String>,
    second: Option<String>,
) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}\n\n{second}")),
        (Some(documentation), None) | (None, Some(documentation)) => Some(documentation),
        (None, None) => None,
    }
}

pub(crate) fn target_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    target_span: ByteSpan,
) -> Option<String> {
    let target_file = analysis.file_by_source(target_span.source)?;
    let target_source = sources.get(target_file.ast.span.source)?;
    let text = target_source.text();
    let symbols = hover_symbols_for_file_analysis(text, target_file);
    let documentation =
        documentation_for_hover_symbols(target_file.ast.span.source, text, &symbols);
    documentation_for_target_span(&documentation, &symbols, target_span)
}

pub(in crate::analysis::hover) fn type_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    if occurrence.kind != crate::analysis::occurrences::SemanticOccurrenceKind::Type {
        return None;
    }
    if let crate::analysis::occurrences::SemanticIdentity::GenericParameter(span) =
        occurrence.identity?
    {
        let parameter = file.typecheck_facts.generic_parameter(span)?;
        return Some(HoverInfo {
            span: occurrence.focus_span,
            label: crate::analysis::presentation::generic_parameter_presentation(
                parameter,
                &file.resolved,
            )
            .render(),
            documentation: None,
        });
    }
    if let crate::analysis::occurrences::SemanticIdentity::Member(span) = occurrence.identity? {
        let (owner, associated) = associated_type_for_declaration_span(analysis, span)?;
        let label = if matches!(occurrence.contextual_type, Some(TypeExpr::Projection(_)))
            || occurrence.role == crate::analysis::occurrences::SemanticOccurrenceRole::Declaration
        {
            format!(
                "associated type {}.{}",
                owner.canonical_name, associated.name
            )
        } else {
            format!(
                "type {}.{} = {}",
                owner.canonical_name,
                associated.name,
                occurrence
                    .contextual_type
                    .as_ref()
                    .map(crate::ast::canonical_type_expr)
                    .unwrap_or_else(|| "<unknown>".to_string())
            )
        };
        return Some(HoverInfo {
            span: occurrence.focus_span,
            label,
            documentation: target_documentation(sources, analysis, associated.name_span),
        });
    }
    let crate::analysis::occurrences::SemanticIdentity::Declaration(declaration_span) =
        occurrence.identity?
    else {
        return None;
    };
    let symbol = type_symbol_for_declaration_span(analysis, declaration_span)?;
    let construction_symbol = file
        .resolved
        .symbol_reference_at_offset(offset)
        .map(|(_, symbol)| symbol)
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Type(_)))
        .unwrap_or(symbol);
    let construction = match &construction_symbol.kind {
        SymbolKind::Type(type_symbol) => {
            crate::analysis::constructions::construction_surface_markdown(
                type_symbol,
                &file.resolved,
            )
        }
        _ => None,
    };
    let coercions = match &construction_symbol.kind {
        SymbolKind::Type(type_symbol) => {
            crate::analysis::coercions::coercion_surface_markdown(type_symbol, &file.resolved)
        }
        _ => None,
    };
    let documentation = combine_documentation(
        combine_documentation(
            combine_documentation(
                target_documentation(sources, analysis, symbol.name_span),
                semantic_documentation(sources, analysis, declaration_span),
            ),
            construction,
        ),
        coercions,
    );
    let presentation = match occurrence.contextual_type.as_ref() {
        Some(contextual_type) => crate::analysis::presentation::type_reference_presentation(
            symbol,
            contextual_type,
            &file.resolved,
        ),
        None => {
            crate::analysis::presentation::type_declaration_presentation(symbol, &file.resolved)
        }
    };
    let label = presentation?.render();

    Some(HoverInfo {
        span: occurrence.focus_span,
        label,
        documentation,
    })
}

fn associated_type_for_declaration_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(
    &crate::resolve::TypeSymbol,
    &crate::resolve::AssociatedTypeSignature,
)> {
    analysis.files.iter().find_map(|file| {
        file.resolved.symbols.symbols().find_map(|symbol| {
            let SymbolKind::Type(owner) = &symbol.kind else {
                return None;
            };
            owner
                .associated_types
                .iter()
                .find(|associated| associated.name_span == declaration_span)
                .map(|associated| (owner, associated))
        })
    })
}

pub(in crate::analysis::hover) fn type_symbol_for_declaration_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<&Symbol> {
    let file = analysis.file_by_source(declaration_span.source)?;
    file.resolved
        .symbols
        .symbols()
        .find(|candidate| is_type_symbol_at_declaration_span(candidate, declaration_span))
}

pub(in crate::analysis::hover) fn is_type_symbol_at_declaration_span(
    symbol: &Symbol,
    declaration_span: ByteSpan,
) -> bool {
    matches!(symbol.kind, SymbolKind::Type(_)) && symbol.declaration_span == declaration_span
}

pub(in crate::analysis::hover) fn documentation_for_hover_symbols(
    source: SourceId,
    text: &str,
    symbols: &[HoverSymbol],
) -> crate::comments::AttachedDocumentation {
    let targets = symbols
        .iter()
        .map(|symbol| DocumentationTarget::new(symbol.attach_start, symbol.target.focus_span.start))
        .collect::<Vec<_>>();
    attach_documentation(source, text, &targets)
}

pub(in crate::analysis::hover) fn documentation_for_target_span(
    documentation: &crate::comments::AttachedDocumentation,
    symbols: &[HoverSymbol],
    target_span: ByteSpan,
) -> Option<String> {
    documentation
        .get(target_span.start)
        .map(str::to_string)
        .or_else(|| {
            symbols
                .iter()
                .find(|symbol| span_contains(symbol.target.declaration_span, target_span.start))
                .and_then(|symbol| documentation.get(symbol.target.focus_span.start))
                .map(str::to_string)
        })
}

pub(in crate::analysis::hover) fn resolved_reference_at_offset(
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let mut candidates = Vec::new();
    if let Some((span, symbol)) = resolved.local_symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::Local(symbol.clone())));
    }
    if let Some((span, symbol)) = resolved.symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::TopLevel(Box::new(symbol.clone()))));
    }
    candidates.sort_by_key(|(span, _)| (span.len(), span.start));
    candidates.into_iter().next()
}

pub(in crate::analysis::hover) fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
