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

pub(in crate::analysis::hover) fn parameters_label(text: &str, parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(in crate::analysis::hover) fn binding_kind_label(
    kind: crate::ast::BindingKind,
) -> &'static str {
    match kind {
        crate::ast::BindingKind::Let => "let",
        crate::ast::BindingKind::Var => "var",
    }
}

pub(in crate::analysis::hover) fn single_file_resolved_reference_hover_contents(
    text: &str,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            single_file_symbol_hover_contents(text, symbols, documentation, symbol)
        }
        ResolvedReference::Local(symbol) => {
            local_symbol_hover_contents(symbols, documentation, symbol)
        }
    }
}

pub(in crate::analysis::hover) fn single_file_symbol_hover_contents(
    text: &str,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    symbol: &Symbol,
) -> (String, Option<String>) {
    let referenced = symbols
        .iter()
        .find(|candidate| candidate.target.declaration_span == symbol.name_span);
    let label = referenced
        .map(|symbol| symbol.label.clone())
        .unwrap_or_else(|| symbol_hover_label(text, symbol));
    let docs = referenced
        .and_then(|symbol| documentation.get(symbol.target.focus_span.start))
        .map(str::to_string);
    (label, docs)
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

pub(in crate::analysis::hover) fn local_symbol_hover_contents(
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    symbol: &LocalSymbol,
) -> (String, Option<String>) {
    let referenced = symbols
        .iter()
        .find(|candidate| candidate.target.declaration_span == symbol.name_span);
    let label = referenced
        .map(|symbol| symbol.label.clone())
        .unwrap_or_else(|| local_symbol_hover_label(symbol));
    let docs = referenced
        .and_then(|symbol| documentation.get(symbol.target.focus_span.start))
        .map(str::to_string);

    (label, docs)
}

pub(in crate::analysis::hover) fn resolved_local_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &LocalSymbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.name_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    let (label, documentation) = local_symbol_hover_contents(&symbols, &documentation, symbol);
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
        LocalSymbolKind::ForRange | LocalSymbolKind::LiteralPackFor => {
            format!("for {}", symbol.name)
        }
        LocalSymbolKind::LiteralCapture => format!("literal pack {}", symbol.name),
    }
}

pub(in crate::analysis::hover) fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.declaration_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let target_name_span = file
        .resolved
        .symbols
        .symbols()
        .find(|candidate| candidate.declaration_span == symbol.declaration_span)
        .map(|candidate| candidate.name_span);
    let hover_symbol = symbols
        .iter()
        .find(|candidate| candidate.target.declaration_span == symbol.declaration_span)
        .or_else(|| {
            target_name_span.and_then(|name_span| {
                symbols
                    .iter()
                    .find(|candidate| candidate.target.declaration_span == name_span)
            })
        })
        .or_else(|| {
            symbols
                .iter()
                .find(|candidate| candidate.target.declaration_span == symbol.name_span)
        })?;
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);
    let docs = documentation
        .get(hover_symbol.target.focus_span.start)
        .map(str::to_string);

    Some((
        hover_symbol.label.clone(),
        combine_documentation(
            docs,
            semantic_documentation(sources, analysis, symbol.declaration_span),
        ),
    ))
}

pub(in crate::analysis::hover) fn symbol_hover_label(text: &str, symbol: &Symbol) -> String {
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
                    parameter_signatures_label(text, &signature.parameters),
                    source_fragment(text, signature.return_type.span())
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
                        source_fragment(text, target.span())
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

pub(in crate::analysis::hover) fn parameter_signatures_label(
    text: &str,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
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
