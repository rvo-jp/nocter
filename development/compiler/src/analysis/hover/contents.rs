use super::*;

pub(in crate::analysis::hover) fn function_like_header(
    text: &str,
    span: ByteSpan,
    body_start: Option<usize>,
) -> String {
    let end = body_start.unwrap_or(span.end).min(span.end);
    source_fragment(text, ByteSpan::new(span.source, span.start, end))
        .trim_end_matches('{')
        .trim()
        .to_string()
}

pub(in crate::analysis::hover) fn parameter_labels(
    text: &str,
    parameters: &[Parameter],
) -> Vec<String> {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect()
}

pub(in crate::analysis::hover) fn binding_kind_label(
    kind: crate::ast::BindingKind,
) -> &'static str {
    match kind {
        crate::ast::BindingKind::Let => "let",
        crate::ast::BindingKind::Var => "var",
    }
}

pub(in crate::analysis::hover) fn resolved_reference_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            resolved_symbol_hover_contents(sources, analysis, symbol).unwrap_or_else(|| {
                (
                    symbol_hover_label_for_sources(sources, symbol),
                    None::<String>,
                )
            })
        }
        ResolvedReference::Local(symbol) => {
            resolved_local_symbol_hover_contents(sources, analysis, symbol)
                .unwrap_or_else(|| (local_symbol_hover_label(symbol), None))
        }
    }
}

pub(in crate::analysis::hover) fn resolved_local_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &LocalSymbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.name_span.source)?;
    let label = crate::analysis::presentation::local_presentation(
        symbol,
        file.typecheck_facts.binding_type_expr(symbol.name_span),
        &file.resolved,
    )
    .render();
    let documentation = target_documentation(sources, analysis, symbol.name_span);
    let region = matches!(symbol.kind, LocalSymbolKind::Region)
        .then(|| crate::analysis::regions::region_markdown(sources, file, symbol.name_span))
        .flatten();

    Some((label, combine_documentation(documentation, region)))
}

pub(in crate::analysis::hover) fn local_symbol_hover_label(symbol: &LocalSymbol) -> String {
    match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter {}", symbol.name),
        LocalSymbolKind::Binding(kind) => format!("{} {}", binding_kind_label(kind), symbol.name),
        LocalSymbolKind::Region => format!("region {}", symbol.name),
        LocalSymbolKind::PatternPayload => format!("payload {}", symbol.name),
        LocalSymbolKind::CatchError => format!("catch {}", symbol.name),
        LocalSymbolKind::ForRange
        | LocalSymbolKind::CollectionFor
        | LocalSymbolKind::LiteralPackFor => {
            format!("for {}", symbol.name)
        }
        LocalSymbolKind::LiteralCapture => format!("literal pack {}", symbol.name),
        LocalSymbolKind::ClosureCapture(mode) => {
            format!("capture {}{}", mode.source_prefix(), symbol.name)
        }
    }
}

pub(in crate::analysis::hover) fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.declaration_span.source)?;
    let declaration = file
        .resolved
        .symbols
        .symbols()
        .find(|candidate| candidate.declaration_span == symbol.declaration_span)?;
    let label = match &declaration.kind {
        SymbolKind::Function(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "func",
                &symbol.name,
                signature,
                &file.resolved,
            )
            .render()
        }
        SymbolKind::Primitive(signature) => {
            crate::analysis::presentation::callable_signature_presentation(
                "primitive",
                &symbol.name,
                signature,
                &file.resolved,
            )
            .render()
        }
        SymbolKind::Type(_) => {
            let mut displayed = declaration.clone();
            displayed.name = symbol.name.clone();
            crate::analysis::presentation::type_declaration_presentation(
                &displayed,
                &file.resolved,
            )?
            .render()
        }
        SymbolKind::Imported(_) => return None,
    };

    Some((
        label,
        combine_documentation(
            target_documentation(sources, analysis, declaration.name_span),
            semantic_documentation(sources, analysis, symbol.declaration_span),
        ),
    ))
}

pub(in crate::analysis::hover) fn symbol_hover_label_for_sources(
    sources: &SourceMap,
    symbol: &Symbol,
) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => {
            append_signature_provenance(
                format!(
                    "{} {}({}): {}",
                    if matches!(&symbol.kind, SymbolKind::Primitive(_)) {
                        "primitive"
                    } else {
                        "func"
                    },
                    symbol.name,
                    parameter_signatures_label_for_sources(sources, &signature.parameters),
                    source_fragment_from_sources(sources, signature.return_type.span())
                ),
                signature.result_provenance.as_ref(),
            )
        }
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => type_symbol
                .alias_target
                .as_ref()
                .map(|target| {
                    format!(
                        "type {} = {}",
                        symbol.name,
                        source_fragment_from_sources(sources, target.span())
                    )
                })
                .unwrap_or_else(|| format!("type {}", symbol.name)),
            TypeSymbolKind::Struct => format!("struct {}", symbol.name),
            TypeSymbolKind::Enum => format!("enum {}", symbol.name),
            TypeSymbolKind::Interface => format!("interface {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn append_signature_provenance(
    mut label: String,
    clause: Option<&crate::ast::ResultProvenanceClause>,
) -> String {
    if let Some(clause) = clause {
        label.push_str(" from ");
        label.push_str(
            &clause
                .origins
                .iter()
                .map(|origin| origin.kind.source_label())
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    label
}

pub(in crate::analysis::hover) fn parameter_signatures_label_for_sources(
    sources: &SourceMap,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment_from_sources(sources, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(in crate::analysis::hover) fn source_fragment_from_sources(
    sources: &SourceMap,
    span: ByteSpan,
) -> String {
    sources
        .get(span.source)
        .map(|source| source_fragment(source.text(), span).to_string())
        .unwrap_or_default()
}

pub(in crate::analysis::hover) fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

pub(in crate::analysis::hover) fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    let mut start = line_start;
    while start < node_start && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }

    start
}
