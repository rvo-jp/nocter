//! Find-references queries derived from compile-unit analysis.

use super::scoped_imports::scoped_import_name_spans;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::AstFile;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbol};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::{TypecheckFacts, collect_typecheck_facts};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceTarget {
    Local(ByteSpan),
    Declaration(ByteSpan),
    Member(ByteSpan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReferenceCandidate {
    span: ByteSpan,
    target: ReferenceTarget,
}

pub(crate) fn reference_spans_for_file_analysis(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
    include_declaration: bool,
) -> Vec<ByteSpan> {
    let Some(target) = selected_reference_target(file, offset) else {
        return Vec::new();
    };

    reference_spans_for_target(analysis.files.iter(), target, include_declaration)
}

pub(crate) fn reference_spans_for_text(
    text: &str,
    offset: usize,
    include_declaration: bool,
) -> Option<Vec<ByteSpan>> {
    let parsed = parse_single_file_text("references.nct", text)?;
    let resolved = resolve_single_file_ast("references.nct", text, parsed.source, &parsed.ast);
    let facts = collect_typecheck_facts(&parsed.ast, &resolved);
    let file = SingleFileAnalysis {
        ast: &parsed.ast,
        resolved: &resolved,
        facts: &facts,
    };
    let target = selected_reference_target_for_parts(file.ast, file.resolved, file.facts, offset)?;
    let spans = reference_spans_for_single_file(file, target, include_declaration);

    Some(spans)
}

fn selected_reference_target(file: &FileAnalysis, offset: usize) -> Option<ReferenceTarget> {
    selected_reference_target_for_parts(&file.ast, &file.resolved, &file.typecheck_facts, offset)
}

fn selected_reference_target_for_parts(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<ReferenceTarget> {
    let mut candidates = Vec::new();

    push_member_reference_candidates(facts, offset, &mut candidates);
    push_function_call_reference_candidates(facts, offset, &mut candidates);
    push_type_reference_candidates(facts, offset, &mut candidates);
    push_resolved_reference_candidates(resolved, offset, &mut candidates);
    push_declaration_candidates(ast.span.source, resolved, offset, &mut candidates);

    candidates.sort_by_key(|candidate| (candidate.span.len(), candidate.span.start));
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.target)
}

fn push_member_reference_candidates(
    facts: &TypecheckFacts,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    if let Some((span, target)) = facts.field_target_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Member(target),
        });
    }
    if let Some((span, target)) = facts.associated_function_target_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Member(target),
        });
    }
    if let Some((span, target)) = facts.enum_variant_target_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Member(target),
        });
    }
    for span in facts.method_call_spans() {
        if span_contains(span, offset)
            && let Some(target) = facts.method_call_target(span)
        {
            candidates.push(ReferenceCandidate {
                span,
                target: ReferenceTarget::Member(target),
            });
        }
    }
}

fn push_function_call_reference_candidates(
    facts: &TypecheckFacts,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    if let Some((span, target)) = facts.function_call_target_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Declaration(target),
        });
    }
}

fn push_type_reference_candidates(
    facts: &TypecheckFacts,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    let Some(reference) = facts.type_reference_at_offset(offset) else {
        return;
    };
    let Some(declaration_span) = reference.symbol_declaration_span else {
        return;
    };
    candidates.push(ReferenceCandidate {
        span: reference.span,
        target: ReferenceTarget::Declaration(declaration_span),
    });
}

fn push_resolved_reference_candidates(
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    if let Some((span, symbol)) = resolved.local_symbol_reference_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Local(symbol.name_span),
        });
    }
    if let Some((span, symbol)) = resolved.symbol_reference_at_offset(offset) {
        candidates.push(ReferenceCandidate {
            span,
            target: ReferenceTarget::Declaration(symbol.declaration_span),
        });
    }
}

fn push_declaration_candidates(
    source: SourceId,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    for local in resolved.local_symbols() {
        if span_contains(local.name_span, offset) {
            candidates.push(ReferenceCandidate {
                span: local.name_span,
                target: ReferenceTarget::Local(local.name_span),
            });
        }
    }

    for symbol in resolved.symbols.symbols() {
        if span_contains(symbol.name_span, offset) {
            candidates.push(ReferenceCandidate {
                span: symbol.name_span,
                target: ReferenceTarget::Declaration(symbol.declaration_span),
            });
        }
        push_member_declaration_candidates(source, symbol, offset, candidates);
    }
}

fn push_member_declaration_candidates(
    source: SourceId,
    symbol: &Symbol,
    offset: usize,
    candidates: &mut Vec<ReferenceCandidate>,
) {
    let SymbolKind::Type(type_symbol) = &symbol.kind else {
        return;
    };

    for span in member_name_spans(type_symbol).filter(|span| span.source == source) {
        if span_contains(span, offset) {
            candidates.push(ReferenceCandidate {
                span,
                target: ReferenceTarget::Member(span),
            });
        }
    }
}

fn reference_spans_for_target<'a>(
    files: impl Iterator<Item = &'a FileAnalysis>,
    target: ReferenceTarget,
    include_declaration: bool,
) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    for file in files {
        collect_reference_spans_for_file(file, target, include_declaration, &mut spans);
    }
    sort_and_dedup_spans(spans)
}

#[derive(Debug, Clone, Copy)]
struct SingleFileAnalysis<'a> {
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
    facts: &'a TypecheckFacts,
}

fn reference_spans_for_single_file(
    file: SingleFileAnalysis<'_>,
    target: ReferenceTarget,
    include_declaration: bool,
) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    collect_reference_spans_for_parts(
        file.ast,
        file.resolved,
        file.facts,
        target,
        include_declaration,
        &mut spans,
    );
    sort_and_dedup_spans(spans)
}

fn collect_reference_spans_for_file(
    file: &FileAnalysis,
    target: ReferenceTarget,
    include_declaration: bool,
    spans: &mut Vec<ByteSpan>,
) {
    collect_reference_spans_for_parts(
        &file.ast,
        &file.resolved,
        &file.typecheck_facts,
        target,
        include_declaration,
        spans,
    );
}

fn collect_reference_spans_for_parts(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    target: ReferenceTarget,
    include_declaration: bool,
    spans: &mut Vec<ByteSpan>,
) {
    match target {
        ReferenceTarget::Local(name_span) => {
            if include_declaration && name_span.source == ast.span.source {
                spans.push(name_span);
            }
            spans.extend(
                resolved
                    .local_symbol_identifier_references()
                    .filter_map(|(span, symbol)| (symbol.name_span == name_span).then_some(span)),
            );
        }
        ReferenceTarget::Declaration(declaration_span) => {
            let scoped_import_spans = scoped_import_name_spans(ast);
            if include_declaration {
                spans.extend(declaration_name_spans(resolved, declaration_span));
            }
            spans.extend(
                resolved
                    .symbol_identifier_references()
                    .filter_map(|(span, symbol)| {
                        (symbol.declaration_span == declaration_span).then_some(span)
                    }),
            );
            spans.extend(imported_name_spans(
                resolved,
                declaration_span,
                &scoped_import_spans,
            ));
            spans.extend(
                facts
                    .function_call_target_spans()
                    .filter(|span| facts.function_call_target(*span) == Some(declaration_span)),
            );
            spans.extend(facts.type_references().filter_map(|reference| {
                (reference.symbol_declaration_span == Some(declaration_span))
                    .then_some(reference.span)
            }));
        }
        ReferenceTarget::Member(name_span) => {
            if include_declaration && name_span.source == ast.span.source {
                spans.push(name_span);
            }
            spans.extend(
                facts
                    .field_target_spans()
                    .filter(|span| facts.field_target(*span) == Some(name_span)),
            );
            spans.extend(
                facts
                    .associated_function_target_spans()
                    .filter(|span| facts.associated_function_target(*span) == Some(name_span)),
            );
            spans.extend(
                facts
                    .enum_variant_target_spans()
                    .filter(|span| facts.enum_variant_target(*span) == Some(name_span)),
            );
            spans.extend(
                facts
                    .method_call_spans()
                    .filter(|span| facts.method_call_target(*span) == Some(name_span)),
            );
        }
    }
}

fn declaration_name_spans(
    resolved: &ResolveOutput,
    declaration_span: ByteSpan,
) -> impl Iterator<Item = ByteSpan> + '_ {
    resolved.symbols.symbols().filter_map(move |symbol| {
        (!symbol.is_hidden
            && symbol.declaration_span == declaration_span
            && symbol.name_span.source == declaration_span.source)
            .then_some(symbol.name_span)
    })
}

fn imported_name_spans<'a>(
    resolved: &'a ResolveOutput,
    declaration_span: ByteSpan,
    scoped_import_spans: &'a HashSet<ByteSpan>,
) -> impl Iterator<Item = ByteSpan> + 'a {
    resolved.symbols.symbols().filter_map(move |symbol| {
        ((!symbol.is_hidden || scoped_import_spans.contains(&symbol.name_span))
            && symbol.declaration_span == declaration_span
            && symbol.name_span.source != declaration_span.source)
            .then_some(symbol.name_span)
    })
}

fn member_name_spans(symbol: &TypeSymbol) -> impl Iterator<Item = ByteSpan> + '_ {
    symbol
        .fields
        .iter()
        .map(|field| field.name_span)
        .chain(symbol.variants.iter().map(|variant| variant.name_span))
        .chain(
            symbol
                .associated_functions
                .iter()
                .map(|function| function.name_span),
        )
        .chain(symbol.methods.iter().map(|method| method.name_span))
        .chain(symbol.drop_member.iter().map(|drop_| drop_.name_span))
}

fn sort_and_dedup_spans(mut spans: Vec<ByteSpan>) -> Vec<ByteSpan> {
    spans.sort_by_key(|span| (span.source.raw(), span.start, span.end));
    spans.dedup_by_key(|span| (span.source.raw(), span.start, span.end));
    spans
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{
        analyze_namespace_import_text, analyze_text, span_fragments_from_sources,
    };

    #[test]
    fn reference_query_finds_local_binding_references() {
        let text = "func main(): i32 {\n    let code = 0\n    return code + code\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("code = 0").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["code", "code", "code"]);
    }

    #[test]
    fn reference_query_finds_top_level_function_references() {
        let text = "func answer(): i32 {\n    return 1\n}\n\nfunc main(): i32 {\n    return answer() + answer()\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("answer():").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["answer", "answer", "answer"]);
    }

    #[test]
    fn reference_query_finds_namespace_imported_function_member_calls() {
        let root_text =
            "use lib/math\n\nfunc main(): i32 {\n    return math.answer() + math.answer()\n}\n";
        let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.find("answer()").expect("expected namespace call");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments_from_sources(&sources, &spans);

        assert_eq!(fragments, vec!["answer", "answer", "answer"]);
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn reference_query_finds_type_references() {
        let text =
            "struct File {\n    fd: i32\n}\n\nfunc open(file: File): File {\n    return file\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("File {").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["File", "File", "File"]);
    }

    #[test]
    fn reference_query_finds_member_references() {
        let text = "struct File {\n    fd: i32\n}\n\nimpl File {\n    method &self.read(): i32 {\n        return self.fd\n    }\n}\n\nfunc main(): i32 {\n    let file = File{ fd: 1 }\n    return file.fd + file.read()\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let field_offset = text.find("fd: i32").expect("expected field");
        let method_offset = text.find("read():").expect("expected method");

        let field_spans = reference_spans_for_file_analysis(&analysis, file, field_offset, true);
        let method_spans = reference_spans_for_file_analysis(&analysis, file, method_offset, true);

        assert_eq!(
            span_fragments(text, &field_spans),
            vec!["fd", "fd", "fd", "fd"]
        );
        assert_eq!(span_fragments(text, &method_spans), vec!["read", "read"]);
    }

    fn span_fragments<'a>(text: &'a str, spans: &[ByteSpan]) -> Vec<&'a str> {
        spans
            .iter()
            .map(|span| &text[span.start..span.end])
            .collect()
    }
}
