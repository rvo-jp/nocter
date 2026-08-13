use super::*;
use crate::ast::TypeExpr;

pub(in crate::analysis::hover) fn syntax_site_hover_for_file_analysis(
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let requirement = file.syntax.coercion_requirement_at(offset)?;
    Some(HoverInfo {
        span: requirement.focus_span,
        label: format!(
            "where {} as {}",
            crate::typecheck::type_expr_presentation_label(&requirement.source, &file.resolved),
            crate::typecheck::type_expr_presentation_label(&requirement.target, &file.resolved),
        ),
        documentation: None,
    })
}

pub(in crate::analysis::hover) fn semantic_declaration_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    let record = analysis.semantic_db.definition(definition)?;
    match record.kind {
        crate::semantic::DefinitionKind::Test => {
            let source = sources.get(record.anchor.source)?;
            Some(HoverInfo {
                span: occurrence.focus_span,
                label: format!(
                    "test {}: void!",
                    source.text().get(record.anchor.start..record.anchor.end)?
                ),
                documentation: target_documentation(sources, analysis, record.anchor),
            })
        }
        crate::semantic::DefinitionKind::Coercion => {
            let crate::resolve::ResolvedDeclaration::Coercion(coercion) =
                file.resolved.declaration(definition)?
            else {
                return None;
            };
            let visibility = if coercion.visibility.is_private() {
                String::new()
            } else {
                format!("{} ", coercion.visibility.source_notation())
            };
            let provenance = coercion
                .result_provenance
                .as_ref()
                .map(|_| " from self")
                .unwrap_or_default();
            Some(HoverInfo {
                span: occurrence.focus_span,
                label: format!(
                    "{visibility}coerce {}self as {}{provenance}",
                    coercion.receiver.mode.source_prefix(),
                    crate::typecheck::type_expr_presentation_label(
                        &coercion.target,
                        &file.resolved,
                    ),
                ),
                documentation: target_documentation(sources, analysis, record.anchor),
            })
        }
        _ => None,
    }
}

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
    let target = definition_anchor(analysis, occurrence.identity);
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
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    match file.resolved.declaration(definition)? {
        crate::resolve::ResolvedDeclaration::Field(owner, field) => {
            let owner_label =
                crate::analysis::presentation::type_owner_presentation_label(owner, &file.resolved);
            let ty = file
                .typed_hir
                .field_type_expr(occurrence.focus_span)
                .unwrap_or(&field.ty);
            Some(HoverInfo {
                span: occurrence.focus_span,
                label: field_member_label(
                    &owner_label,
                    &field.name,
                    &crate::typecheck::type_expr_presentation_label(ty, &file.resolved),
                ),
                documentation: target_documentation(sources, analysis, field.name_span),
            })
        }
        crate::resolve::ResolvedDeclaration::Variant(owner, variant) => {
            let owner_label =
                crate::analysis::presentation::type_owner_presentation_label(owner, &file.resolved);
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
            Some(HoverInfo {
                span: occurrence.focus_span,
                label: enum_variant_member_label(&owner_label, &variant.name, &payload),
                documentation: target_documentation(sources, analysis, variant.name_span),
            })
        }
        crate::resolve::ResolvedDeclaration::Method(owner, method) => Some(HoverInfo {
            span: occurrence.focus_span,
            label: crate::analysis::presentation::method_or_operator_presentation(
                owner,
                method,
                &file.resolved,
            ),
            documentation: combine_documentation(
                target_documentation(sources, analysis, method.name_span),
                semantic_documentation(sources, analysis, method.name_span),
            ),
        }),
        _ => None,
    }
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
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    let crate::resolve::ResolvedDeclaration::Literal(owner, literal) =
        file.resolved.declaration(definition)?
    else {
        return None;
    };
    Some(HoverInfo {
        span: occurrence.focus_span,
        label: crate::analysis::presentation::literal_signature_presentation(
            owner,
            literal,
            &file.resolved,
        )
        .render(),
        documentation: combine_documentation(
            target_documentation(sources, analysis, literal.shape_span),
            semantic_documentation(sources, analysis, literal.declaration_span),
        ),
    })
}

pub(in crate::analysis::hover) fn local_occurrence_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let occurrence = file.occurrences.at_offset(offset)?;
    let crate::analysis::occurrences::SemanticIdentity::Local(symbol_id) = occurrence.identity?
    else {
        return None;
    };
    let target_file = analysis.file_by_source(occurrence.focus_span.source)?;
    let symbol = target_file.resolved.local_symbol(symbol_id)?;
    let target = symbol.name_span;
    let label = crate::analysis::presentation::local_presentation(
        symbol,
        target_file.typed_hir.binding_type_expr(target),
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
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    if analysis.semantic_db.definition(definition)?.kind
        == crate::semantic::DefinitionKind::AssociatedFunction
    {
        return None;
    }
    let target = analysis.semantic_db.definition_anchor(definition)?;
    let crate::resolve::ResolvedDeclaration::Symbol(symbol) =
        file.resolved.declaration(definition)?
    else {
        return None;
    };
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
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    match file.resolved.declaration(definition)? {
        crate::resolve::ResolvedDeclaration::AssociatedFunction(owner, function) => {
            Some(HoverInfo {
                span: occurrence.focus_span,
                label: crate::analysis::presentation::associated_function_presentation(
                    owner,
                    function,
                    &file.resolved,
                )
                .render(),
                documentation: combine_documentation(
                    target_documentation(sources, analysis, function.name_span),
                    semantic_documentation(sources, analysis, function.name_span),
                ),
            })
        }
        crate::resolve::ResolvedDeclaration::Method(owner, method) => Some(HoverInfo {
            span: occurrence.focus_span,
            label: crate::analysis::presentation::method_or_operator_presentation(
                owner,
                method,
                &file.resolved,
            ),
            documentation: combine_documentation(
                target_documentation(sources, analysis, method.name_span),
                semantic_documentation(sources, analysis, method.name_span),
            ),
        }),
        crate::resolve::ResolvedDeclaration::Destructor(owner, drop_) => Some(HoverInfo {
            span: occurrence.focus_span,
            label: crate::analysis::presentation::drop_presentation(owner, drop_, &file.resolved),
            documentation: target_documentation(sources, analysis, drop_.name_span),
        }),
        _ => None,
    }
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
    _sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    target_span: ByteSpan,
) -> Option<String> {
    let target_file = analysis.file_by_source(target_span.source)?;
    target_file
        .syntax
        .documentation_at(target_span)
        .map(str::to_string)
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
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) =
        occurrence.identity?
    else {
        return None;
    };
    let semantic_definition = analysis.semantic_db.definition(definition)?;
    let declaration_span = semantic_definition
        .owner
        .map_or(semantic_definition.span, |_| semantic_definition.anchor);
    if semantic_definition.kind == crate::semantic::DefinitionKind::GenericParameter {
        let span = declaration_span;
        let parameter = file.typed_hir.generic_parameter(span)?;
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
    if semantic_definition.kind == crate::semantic::DefinitionKind::AssociatedType {
        let crate::resolve::ResolvedDeclaration::AssociatedType(owner, associated) =
            file.resolved.declaration(definition)?
        else {
            return None;
        };
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
    let crate::resolve::ResolvedDeclaration::Symbol(symbol) =
        file.resolved.declaration(definition)?
    else {
        return None;
    };
    if !matches!(symbol.kind, SymbolKind::Type(_)) {
        return None;
    }
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

fn definition_anchor(
    analysis: &CompileUnitAnalysis,
    identity: Option<crate::analysis::occurrences::SemanticIdentity>,
) -> Option<ByteSpan> {
    let crate::analysis::occurrences::SemanticIdentity::Definition(definition) = identity? else {
        return None;
    };
    let definition = analysis.semantic_db.definition(definition)?;
    Some(match definition.kind {
        crate::semantic::DefinitionKind::Function if definition.owner.is_none() => definition.span,
        crate::semantic::DefinitionKind::Primitive
        | crate::semantic::DefinitionKind::TypeAlias
        | crate::semantic::DefinitionKind::Struct
        | crate::semantic::DefinitionKind::Enum
        | crate::semantic::DefinitionKind::Interface => definition.span,
        _ => definition.anchor,
    })
}
