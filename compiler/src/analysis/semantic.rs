//! Semantic identifier classification shared by editor tooling.

use super::FileAnalysis;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::ast::BindingKind;
use crate::resolve::{
    FunctionSignature, LocalSymbol, LocalSymbolKind, ResolveOutput, SymbolKind, TypeSymbol,
};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::{TypecheckFacts, collect_typecheck_facts};

pub(crate) const SEMANTIC_DECLARATION_MODIFIER: u32 = 1 << 0;
pub(crate) const SEMANTIC_READONLY_MODIFIER: u32 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticTokenKind {
    Function,
    Method,
    Variable,
    Parameter,
    Type,
    Property,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassifiedIdentifier {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) kind: SemanticTokenKind,
    pub(crate) modifiers: u32,
}

pub(crate) fn classified_identifiers_for_file_analysis(
    _text: &str,
    file: &FileAnalysis,
) -> Vec<ClassifiedIdentifier> {
    classified_identifiers_for_analysis(&file.ast, &file.resolved, &file.typecheck_facts)
}

pub(crate) fn classified_identifiers_for_single_file_text(
    text: &str,
) -> Option<Vec<ClassifiedIdentifier>> {
    let parsed = parse_single_file_text("semantic.nct", text)?;
    let resolved = resolve_single_file_ast("semantic.nct", text, parsed.source, &parsed.ast);
    let facts = collect_typecheck_facts(&parsed.ast, &resolved);

    Some(classified_identifiers_for_analysis(
        &parsed.ast,
        &resolved,
        &facts,
    ))
}

fn classified_identifiers_for_analysis(
    ast: &crate::ast::AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
) -> Vec<ClassifiedIdentifier> {
    let mut collector = SemanticIdentifierCollector {
        source: ast.span.source,
        resolved,
        facts,
        identifiers: Vec::new(),
    };
    collector.collect();
    collector.finish()
}

struct SemanticIdentifierCollector<'a> {
    source: SourceId,
    resolved: &'a ResolveOutput,
    facts: &'a TypecheckFacts,
    identifiers: Vec<ClassifiedIdentifier>,
}

impl SemanticIdentifierCollector<'_> {
    fn collect(&mut self) {
        self.collect_symbol_declarations();
        self.collect_local_symbol_declarations();
        self.collect_symbol_references();
        self.collect_local_symbol_references();
        self.collect_type_references();
        self.collect_member_references();
    }

    fn finish(mut self) -> Vec<ClassifiedIdentifier> {
        self.identifiers.sort_by(|left, right| {
            left.start_byte
                .cmp(&right.start_byte)
                .then(left.end_byte.cmp(&right.end_byte))
        });
        self.identifiers
    }

    fn collect_symbol_declarations(&mut self) {
        for symbol in self.resolved.symbols.symbols() {
            if symbol.is_hidden {
                continue;
            }
            match &symbol.kind {
                SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => {
                    self.push(symbol.name_span, SemanticTokenKind::Function, true, 0);
                    self.collect_function_signature_parameters(signature);
                }
                SymbolKind::Type(type_symbol) => {
                    self.push(symbol.name_span, SemanticTokenKind::Type, true, 0);
                    self.collect_type_symbol_declarations(type_symbol);
                }
                SymbolKind::Imported(_) => {}
            }
        }
    }

    fn collect_type_symbol_declarations(&mut self, symbol: &TypeSymbol) {
        for field in &symbol.fields {
            self.push(field.name_span, SemanticTokenKind::Property, true, 0);
        }

        for variant in &symbol.variants {
            self.push(variant.name_span, SemanticTokenKind::Property, true, 0);
            for parameter in &variant.payload {
                self.push_parameter(parameter.name_span);
            }
        }

        for function in &symbol.associated_functions {
            self.push(function.name_span, SemanticTokenKind::Function, true, 0);
            self.collect_function_signature_parameters(&function.signature);
        }

        for method in &symbol.methods {
            self.push(method.name_span, SemanticTokenKind::Method, true, 0);
            self.push_parameter(method.receiver.name_span);
            self.collect_function_signature_parameters(&method.signature);
        }

        if let Some(drop_) = &symbol.drop_member {
            self.push_parameter(drop_.binding.name_span);
        }
    }

    fn collect_function_signature_parameters(&mut self, signature: &FunctionSignature) {
        for parameter in &signature.parameters {
            self.push_parameter(parameter.name_span);
        }
    }

    fn collect_local_symbol_declarations(&mut self) {
        for symbol in self.resolved.local_symbols() {
            self.push_local_symbol(symbol.name_span, symbol);
        }
    }

    fn collect_symbol_references(&mut self) {
        for (span, symbol) in self.resolved.symbol_identifier_references() {
            let Some(kind) = semantic_kind_for_symbol_kind(&symbol.kind) else {
                continue;
            };
            self.push(span, kind, false, 0);
        }
    }

    fn collect_local_symbol_references(&mut self) {
        for (span, symbol) in self.resolved.local_symbol_identifier_references() {
            self.push_local_symbol(span, symbol);
        }
    }

    fn collect_type_references(&mut self) {
        let spans = self.facts.type_reference_spans().collect::<Vec<_>>();
        for span in spans {
            self.push(span, SemanticTokenKind::Type, false, 0);
        }
    }

    fn collect_member_references(&mut self) {
        let function_spans = self.facts.function_call_target_spans().collect::<Vec<_>>();
        for span in function_spans {
            self.push(span, SemanticTokenKind::Function, false, 0);
        }

        let method_spans = self.facts.method_call_spans().collect::<Vec<_>>();
        for span in method_spans {
            self.push(span, SemanticTokenKind::Method, false, 0);
        }

        let associated_function_spans = self
            .facts
            .associated_function_target_spans()
            .collect::<Vec<_>>();
        for span in associated_function_spans {
            self.push(span, SemanticTokenKind::Function, false, 0);
        }

        let field_spans = self.facts.field_target_spans().collect::<Vec<_>>();
        for span in field_spans {
            let modifiers = if self.facts.field_is_readonly(span) == Some(true) {
                SEMANTIC_READONLY_MODIFIER
            } else {
                0
            };
            self.push(span, SemanticTokenKind::Property, false, modifiers);
        }

        let variant_spans = self.facts.enum_variant_target_spans().collect::<Vec<_>>();
        for span in variant_spans {
            self.push(span, SemanticTokenKind::Property, false, 0);
        }
    }

    fn push_parameter(&mut self, span: ByteSpan) {
        self.push(
            span,
            SemanticTokenKind::Parameter,
            false,
            SEMANTIC_READONLY_MODIFIER,
        );
    }

    fn push_local_symbol(&mut self, span: ByteSpan, symbol: &LocalSymbol) {
        self.push(
            span,
            semantic_kind_for_local_symbol_kind(symbol.kind),
            false,
            local_symbol_modifiers(symbol, span, self.facts),
        );
    }

    fn push(&mut self, span: ByteSpan, kind: SemanticTokenKind, declaration: bool, modifiers: u32) {
        if span.source != self.source || span.is_empty() {
            return;
        }

        let mut modifiers = modifiers;
        if declaration {
            modifiers |= SEMANTIC_DECLARATION_MODIFIER;
        }

        if let Some(identifier) = self.identifiers.iter_mut().find(|identifier| {
            identifier.start_byte == span.start && identifier.end_byte == span.end
        }) {
            identifier.kind = merge_semantic_kind(identifier.kind, kind);
            identifier.modifiers |= modifiers;
            return;
        }

        self.identifiers.push(ClassifiedIdentifier {
            start_byte: span.start,
            end_byte: span.end,
            kind,
            modifiers,
        });
    }
}

fn semantic_kind_for_symbol_kind(kind: &SymbolKind) -> Option<SemanticTokenKind> {
    match kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => Some(SemanticTokenKind::Function),
        SymbolKind::Type(_) => Some(SemanticTokenKind::Type),
        SymbolKind::Imported(_) => None,
    }
}

fn semantic_kind_for_local_symbol_kind(kind: LocalSymbolKind) -> SemanticTokenKind {
    match kind {
        LocalSymbolKind::Parameter => SemanticTokenKind::Parameter,
        LocalSymbolKind::Binding(_)
        | LocalSymbolKind::PatternPayload
        | LocalSymbolKind::CatchError
        | LocalSymbolKind::ForRange => SemanticTokenKind::Variable,
    }
}

fn local_symbol_modifiers(symbol: &LocalSymbol, span: ByteSpan, facts: &TypecheckFacts) -> u32 {
    match symbol.kind {
        LocalSymbolKind::Parameter | LocalSymbolKind::Binding(BindingKind::Let) => {
            SEMANTIC_READONLY_MODIFIER
        }
        LocalSymbolKind::Binding(BindingKind::Var) => 0,
        LocalSymbolKind::PatternPayload
        | LocalSymbolKind::CatchError
        | LocalSymbolKind::ForRange => {
            if facts.binding_is_readonly(span) == Some(true) {
                SEMANTIC_READONLY_MODIFIER
            } else {
                0
            }
        }
    }
}

fn merge_semantic_kind(
    existing: SemanticTokenKind,
    incoming: SemanticTokenKind,
) -> SemanticTokenKind {
    if semantic_kind_priority(incoming) > semantic_kind_priority(existing) {
        incoming
    } else {
        existing
    }
}

const fn semantic_kind_priority(kind: SemanticTokenKind) -> u8 {
    match kind {
        SemanticTokenKind::Property => 5,
        SemanticTokenKind::Method => 4,
        SemanticTokenKind::Function => 3,
        SemanticTokenKind::Type => 2,
        SemanticTokenKind::Parameter => 1,
        SemanticTokenKind::Variable => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{analyze_namespace_import_text, analyze_text};

    #[test]
    fn single_file_analysis_classifies_builtin_types() {
        let text = "func main(path: &str): void! {\n    let byte: u8 = 0 as u8\n    return\n}\n";
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");

        for name in ["str", "void", "u8"] {
            assert!(
                identifiers_for_lexeme(text, &identifiers, name)
                    .iter()
                    .any(|identifier| identifier.kind == SemanticTokenKind::Type),
                "expected `{name}` to be classified as a type"
            );
        }
    }

    #[test]
    fn single_file_analysis_classifies_associated_function_members() {
        let text =
            "struct Point {\n}\n\nfunc Point.origin(): Point {\n    return Point.origin()\n}\n";
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");

        assert!(
            identifiers_for_lexeme(text, &identifiers, "Point")
                .iter()
                .all(|identifier| identifier.kind == SemanticTokenKind::Type),
            "expected associated function owner names to be classified as types"
        );

        let origin_identifiers = identifiers_for_lexeme(text, &identifiers, "origin");
        assert!(
            origin_identifiers.iter().any(|identifier| {
                identifier.kind == SemanticTokenKind::Function
                    && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0
            }),
            "expected associated function declaration name to be a function declaration"
        );
        assert!(
            origin_identifiers.iter().any(|identifier| {
                identifier.kind == SemanticTokenKind::Function
                    && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER == 0
            }),
            "expected associated function call name to be a function"
        );
    }

    #[test]
    fn analysis_classifies_namespace_imported_function_member_calls() {
        let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
        let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        let answer = identifier_starting_at(
            &identifiers,
            root_text.find("answer()").expect("expected namespace call"),
        )
        .expect("expected namespace member call token");

        assert_eq!(answer.kind, SemanticTokenKind::Function);
        assert_eq!(answer.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);
    }

    #[test]
    fn analysis_classification_uses_typecheck_facts() {
        let text = "func main(path: &str): i32 {\n    let alpha = 1\n    var beta = 2\n    return alpha + beta\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        assert!(
            identifiers_for_lexeme(text, &identifiers, "alpha")
                .iter()
                .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER != 0),
            "expected `alpha` to be marked readonly"
        );
        assert!(
            identifiers_for_lexeme(text, &identifiers, "beta")
                .iter()
                .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER == 0),
            "expected `beta` to remain mutable"
        );
    }

    #[test]
    fn analysis_classification_uses_typecheck_facts_for_member_references() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc File.open(): Self {\n    return Self{ fd: 1 }\n}\n\nenum Event {\n    count(value: i32)\n}\n\nfunc main(): i32 {\n    let file = File.open()\n    let event = Event.count(1)\n    return file.fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        let open = identifier_starting_at(
            &identifiers,
            text.rfind("open()").expect("expected associated function"),
        )
        .expect("expected associated function token");
        assert_eq!(open.kind, SemanticTokenKind::Function);

        let count = identifier_starting_at(
            &identifiers,
            text.rfind("count(1)").expect("expected enum variant"),
        )
        .expect("expected enum variant token");
        assert_eq!(count.kind, SemanticTokenKind::Property);

        let fd = identifier_starting_at(
            &identifiers,
            text.rfind("fd").expect("expected field reference"),
        )
        .expect("expected field token");
        assert_eq!(fd.kind, SemanticTokenKind::Property);
    }

    fn identifiers_for_lexeme<'a>(
        text: &str,
        identifiers: &'a [ClassifiedIdentifier],
        lexeme: &str,
    ) -> Vec<&'a ClassifiedIdentifier> {
        identifiers
            .iter()
            .filter(|identifier| text[identifier.start_byte..identifier.end_byte] == *lexeme)
            .collect()
    }

    fn identifier_starting_at(
        identifiers: &[ClassifiedIdentifier],
        start_byte: usize,
    ) -> Option<&ClassifiedIdentifier> {
        identifiers
            .iter()
            .find(|identifier| identifier.start_byte == start_byte)
    }
}
