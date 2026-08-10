//! Semantic identifier classification shared by editor tooling.

use super::FileAnalysis;
use super::occurrences::{SemanticOccurrenceIndex, SemanticOccurrenceKind, SemanticOccurrenceRole};
use super::scoped_imports::scoped_import_name_spans;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind, TypeSymbol};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::collect_typecheck_facts;
use std::collections::HashSet;

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
    Namespace,
    Keyword,
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
    classified_identifiers_for_analysis(&file.ast, &file.resolved, &file.occurrences)
}

pub(crate) fn classified_identifiers_for_single_file_text(
    text: &str,
) -> Option<Vec<ClassifiedIdentifier>> {
    if let Some(identifiers) = classify_single_file_text(text) {
        return Some(identifiers);
    }

    if let Some(recovery) = super::collection_for_recovery::collection_for_document_recovery(text) {
        let recovered_text = super::delimiter_recovery::close_unmatched_braces(&recovery.text)
            .unwrap_or(recovery.text);
        if let Some(mut identifiers) = classify_single_file_text(&recovered_text) {
            let inserted_end = recovery.insertion_start + recovery.insertion_len;
            identifiers.retain_mut(|identifier| {
                if identifier.end_byte <= recovery.insertion_start {
                    return true;
                }
                if identifier.start_byte >= inserted_end {
                    identifier.start_byte -= recovery.insertion_len;
                    identifier.end_byte -= recovery.insertion_len;
                    return true;
                }
                false
            });
            return Some(identifiers);
        }
    }

    let recovered = super::delimiter_recovery::block_recovery_text(text, text.len())?;
    classify_single_file_text(&recovered)
}

fn classify_single_file_text(text: &str) -> Option<Vec<ClassifiedIdentifier>> {
    let parsed = parse_single_file_text("semantic.nct", text)?;
    let resolved = resolve_single_file_ast("semantic.nct", text, parsed.source, &parsed.ast);
    let facts = collect_typecheck_facts(&parsed.ast, &resolved);
    let occurrences = SemanticOccurrenceIndex::new(&parsed.ast, &resolved, &facts);

    Some(classified_identifiers_for_analysis(
        &parsed.ast,
        &resolved,
        &occurrences,
    ))
}

fn classified_identifiers_for_analysis(
    ast: &crate::ast::AstFile,
    resolved: &ResolveOutput,
    occurrences: &SemanticOccurrenceIndex,
) -> Vec<ClassifiedIdentifier> {
    let mut collector = SemanticIdentifierCollector {
        ast,
        source: ast.span.source,
        resolved,
        occurrences,
        scoped_import_spans: scoped_import_name_spans(ast),
        identifiers: Vec::new(),
    };
    collector.collect();
    collector.finish()
}

struct SemanticIdentifierCollector<'a> {
    ast: &'a crate::ast::AstFile,
    source: SourceId,
    resolved: &'a ResolveOutput,
    occurrences: &'a SemanticOccurrenceIndex,
    scoped_import_spans: HashSet<ByteSpan>,
    identifiers: Vec<ClassifiedIdentifier>,
}

impl SemanticIdentifierCollector<'_> {
    fn collect(&mut self) {
        self.collect_semantic_occurrences();
        self.collect_test_declarations();
        self.collect_signature_parameter_declarations();
        self.collect_provenance_references();
        self.collect_generic_requirement_keywords();
        self.collect_editor_targets();
    }

    fn collect_test_declarations(&mut self) {
        let declarations = self
            .ast
            .items
            .iter()
            .filter_map(|item| match item {
                crate::ast::Item::Test(test) => Some(test.name_span),
                _ => None,
            })
            .collect::<Vec<_>>();
        for span in declarations {
            self.push(span, SemanticTokenKind::Function, true, 0);
        }
    }

    fn finish(mut self) -> Vec<ClassifiedIdentifier> {
        self.identifiers.sort_by(|left, right| {
            left.start_byte
                .cmp(&right.start_byte)
                .then(left.end_byte.cmp(&right.end_byte))
        });
        self.identifiers
    }

    fn collect_semantic_occurrences(&mut self) {
        for occurrence in self.occurrences.iter() {
            let kind = match occurrence.kind {
                SemanticOccurrenceKind::Function => SemanticTokenKind::Function,
                SemanticOccurrenceKind::Literal => continue,
                SemanticOccurrenceKind::Method => SemanticTokenKind::Method,
                SemanticOccurrenceKind::Variable => SemanticTokenKind::Variable,
                SemanticOccurrenceKind::Parameter => SemanticTokenKind::Parameter,
                SemanticOccurrenceKind::Type => SemanticTokenKind::Type,
                SemanticOccurrenceKind::Property => SemanticTokenKind::Property,
                SemanticOccurrenceKind::Namespace => SemanticTokenKind::Namespace,
            };
            let modifiers = if occurrence.is_readonly {
                SEMANTIC_READONLY_MODIFIER
            } else {
                0
            };
            self.push(
                occurrence.focus_span,
                kind,
                occurrence.role == SemanticOccurrenceRole::Declaration,
                modifiers,
            );
        }
    }

    fn collect_signature_parameter_declarations(&mut self) {
        for symbol in self.resolved.symbols.symbols() {
            if symbol.is_hidden && !self.scoped_import_spans.contains(&symbol.name_span) {
                continue;
            }
            match &symbol.kind {
                SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => {
                    self.collect_function_signature_parameters(signature);
                }
                SymbolKind::Type(type_symbol) => {
                    self.collect_type_symbol_parameter_declarations(type_symbol);
                }
                SymbolKind::Imported(_) => {}
            }
        }
    }

    fn collect_type_symbol_parameter_declarations(&mut self, symbol: &TypeSymbol) {
        for variant in &symbol.variants {
            for parameter in &variant.payload {
                self.push_parameter(parameter.name_span);
            }
        }

        for function in &symbol.associated_functions {
            self.collect_function_signature_parameters(&function.signature);
        }

        for method in &symbol.methods {
            self.push_parameter(method.receiver.name_span);
            self.collect_function_signature_parameters(&method.signature);
        }

        if let Some(drop_) = &symbol.destructor {
            self.push_parameter(drop_.binding.name_span);
        }
    }

    fn collect_function_signature_parameters(&mut self, signature: &FunctionSignature) {
        for parameter in &signature.parameters {
            self.push_parameter(parameter.name_span);
        }
    }

    fn collect_provenance_references(&mut self) {
        let mut spans = Vec::new();
        for item in &self.ast.items {
            match item {
                crate::ast::Item::Function(item) => {
                    collect_provenance_parameter_spans(item.result_provenance.as_ref(), &mut spans);
                }
                crate::ast::Item::Primitive(item) => {
                    collect_provenance_parameter_spans(item.result_provenance.as_ref(), &mut spans);
                }
                crate::ast::Item::Interface(item) => {
                    for method in &item.methods {
                        collect_provenance_parameter_spans(
                            method.result_provenance.as_ref(),
                            &mut spans,
                        );
                    }
                }
                crate::ast::Item::Instance(_) | crate::ast::Item::Conformance(_) => {
                    for method in item.method_owner().expect("matched method owner").methods() {
                        collect_provenance_parameter_spans(
                            method.result_provenance.as_ref(),
                            &mut spans,
                        );
                    }
                }
                crate::ast::Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        collect_provenance_parameter_spans(
                            function.result_provenance.as_ref(),
                            &mut spans,
                        );
                    }
                    for (_, literal) in construct.literals() {
                        collect_provenance_parameter_spans(
                            literal.result_provenance.as_ref(),
                            &mut spans,
                        );
                    }
                }
                crate::ast::Item::Coerce(coerce) => {
                    for entry in &coerce.entries {
                        collect_provenance_parameter_spans(
                            entry.result_provenance.as_ref(),
                            &mut spans,
                        );
                    }
                }
                crate::ast::Item::Import(_)
                | crate::ast::Item::Test(_)
                | crate::ast::Item::FromImport(_)
                | crate::ast::Item::TypeAlias(_)
                | crate::ast::Item::Struct(_)
                | crate::ast::Item::Enum(_) => {}
                crate::ast::Item::Destruct(_) => {}
            }
        }
        for span in spans {
            self.push_parameter(span);
        }
    }

    fn collect_editor_targets(&mut self) {
        for target in
            crate::analysis::editor_targets::editor_targets_for_ast(self.ast, self.resolved)
        {
            let kind = match target.kind {
                crate::analysis::editor_targets::EditorTargetKind::Module(_) => {
                    Some(SemanticTokenKind::Namespace)
                }
                crate::analysis::editor_targets::EditorTargetKind::ImportBinding(symbol) => {
                    semantic_kind_for_symbol_kind(&symbol.kind).or_else(|| {
                        matches!(
                            &symbol.kind,
                            SymbolKind::Imported(imported)
                                if imported.kind == crate::resolve::ImportedSymbolKind::Namespace
                        )
                        .then_some(SemanticTokenKind::Namespace)
                    })
                }
            };
            if let Some(kind) = kind {
                self.push(target.focus_span, kind, true, 0);
            }
        }
    }

    fn collect_generic_requirement_keywords(&mut self) {
        let mut spans = Vec::new();
        for item in &self.ast.items {
            collect_item_generic_requirement_keyword_spans(item, &mut spans);
        }
        for span in spans {
            self.push(span, SemanticTokenKind::Keyword, false, 0);
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

fn collect_provenance_parameter_spans(
    clause: Option<&crate::ast::ResultProvenanceClause>,
    spans: &mut Vec<ByteSpan>,
) {
    let Some(clause) = clause else {
        return;
    };
    spans.extend(clause.origins.iter().filter_map(|origin| {
        matches!(
            origin.kind,
            crate::ast::ResultProvenanceOriginKind::Receiver
                | crate::ast::ResultProvenanceOriginKind::Parameter(_)
        )
        .then_some(origin.span)
    }));
}

fn semantic_kind_for_symbol_kind(kind: &SymbolKind) -> Option<SemanticTokenKind> {
    match kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => Some(SemanticTokenKind::Function),
        SymbolKind::Type(_) => Some(SemanticTokenKind::Type),
        SymbolKind::Imported(_) => None,
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
        SemanticTokenKind::Namespace => 6,
        SemanticTokenKind::Keyword => 7,
    }
}

fn collect_item_generic_requirement_keyword_spans(
    item: &crate::ast::Item,
    spans: &mut Vec<ByteSpan>,
) {
    fn clause(clause: Option<&crate::ast::WhereClause>, spans: &mut Vec<ByteSpan>) {
        if let Some(clause) = clause {
            spans.push(clause.keyword_span);
            spans.extend(
                clause
                    .copy_requirements()
                    .map(|requirement| requirement.keyword_span),
            );
        }
    }
    fn method(method: &crate::ast::MethodDecl, spans: &mut Vec<ByteSpan>) {
        clause(method.requirements.as_ref(), spans);
    }
    match item {
        crate::ast::Item::Function(function) => {
            clause(function.requirements.as_ref(), spans);
        }
        crate::ast::Item::Primitive(primitive) => {
            clause(primitive.requirements.as_ref(), spans);
        }
        crate::ast::Item::TypeAlias(alias) => clause(alias.requirements.as_ref(), spans),
        crate::ast::Item::Struct(struct_) => clause(struct_.requirements.as_ref(), spans),
        crate::ast::Item::Enum(enum_) => clause(enum_.requirements.as_ref(), spans),
        crate::ast::Item::Interface(interface) => {
            clause(interface.requirements.as_ref(), spans);
            for member in &interface.methods {
                method(member, spans);
            }
        }
        crate::ast::Item::Instance(_) | crate::ast::Item::Conformance(_) => {
            let owner = item.method_owner().expect("matched method owner");
            clause(owner.requirements(), spans);
            for member in owner.methods() {
                method(member, spans);
            }
        }
        crate::ast::Item::Construct(construct) => {
            for (_, function) in construct.functions() {
                clause(function.requirements.as_ref(), spans);
            }
            for (_, literal) in construct.literals() {
                clause(literal.requirements.as_ref(), spans);
            }
        }
        crate::ast::Item::Coerce(_) => {}
        crate::ast::Item::Destruct(_) => {}
        crate::ast::Item::Import(_)
        | crate::ast::Item::FromImport(_)
        | crate::ast::Item::Test(_) => {}
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
    fn associated_type_names_are_semantic_types() {
        let text = r#"interface Source { pub type Item }
struct NumberSource { value: i32 }
conform Source for NumberSource { type Item = i32 }
func project<S>(source: S): S.Item where S: Source { return source }
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");
        let item_tokens = identifiers_for_lexeme(text, &identifiers, "Item");
        assert_eq!(item_tokens.len(), 3, "{item_tokens:#?}");
        assert!(
            item_tokens
                .iter()
                .all(|identifier| identifier.kind == SemanticTokenKind::Type)
        );
    }

    #[test]
    fn native_test_name_is_a_function_like_declaration_only_on_its_identifier() {
        let text = "test pushes { return }\n";
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("semantic analysis");
        let start = text.find("pushes").unwrap();
        let test = identifiers
            .iter()
            .find(|identifier| identifier.start_byte == start)
            .expect("test declaration token");
        assert_eq!(test.end_byte, start + "pushes".len());
        assert_eq!(test.kind, SemanticTokenKind::Function);
        assert_ne!(test.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);
        assert!(
            !identifiers
                .iter()
                .any(|identifier| identifier.start_byte == 0)
        );
    }

    #[test]
    fn incomplete_collection_for_recovery_remaps_identifiers_after_the_edit() {
        let text = "func observe(value: i32): void { return }\nfunc run(values: i32): void {\n    for item in &\n    observe(values)\n    return\n}\n";
        let identifiers = classified_identifiers_for_single_file_text(text)
            .expect("expected collection-for semantic recovery");
        let item = text.find("item in").unwrap();
        let values = text.rfind("values)").unwrap();

        assert!(identifiers.iter().any(|identifier| {
            identifier.start_byte == item
                && identifier.end_byte == item + "item".len()
                && identifier.kind == SemanticTokenKind::Variable
        }));
        assert!(
            identifiers.iter().any(|identifier| {
                identifier.start_byte == values
                    && identifier.end_byte == values + "values".len()
                    && identifier.kind == SemanticTokenKind::Parameter
            }),
            "identifiers: {identifiers:#?}"
        );
    }

    #[test]
    fn semantic_identifiers_survive_an_unclosed_member_body() {
        let text = r#"struct Token { value: i32 }

destruct Token(&+self) {
    return
"#;
        let identifiers = classified_identifiers_for_single_file_text(text)
            .expect("expected recovered semantic identifiers");

        let drop_keyword = identifier_starting_at(
            &identifiers,
            text.find("destruct Token")
                .expect("expected destruct declaration"),
        )
        .expect("expected drop semantic token");
        assert_eq!(drop_keyword.kind, SemanticTokenKind::Method);

        let receiver = identifier_starting_at(
            &identifiers,
            text.find("self) {").expect("expected receiver"),
        )
        .expect("expected receiver semantic token");
        assert_eq!(receiver.kind, SemanticTokenKind::Parameter);
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
    fn single_file_analysis_classifies_construct_function_members() {
        let text = r#"struct Bucket<T> { value: T }

construct Bucket<T> {
pub default func new(value: T): Self {
    return Bucket<T> { value: value }
}

func main(): i32 {
    let bucket = Bucket.new(42)
    return 0
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");
        let new_identifiers = identifiers_for_lexeme(text, &identifiers, "new");

        assert!(new_identifiers.iter().any(|identifier| {
            identifier.kind == SemanticTokenKind::Function
                && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0
        }));
        assert!(new_identifiers.iter().any(|identifier| {
            identifier.kind == SemanticTokenKind::Function
                && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER == 0
        }));
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
    fn analysis_classifies_a_whole_module_path_as_one_namespace() {
        let root_text = "use lib/math\n\nfunc main(): i32 { return 0 }\n";
        let module_text = "pub func answer(): i32 { return 7 }\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        let namespace = identifiers_for_lexeme(root_text, &identifiers, "lib/math");
        assert_eq!(namespace.len(), 1);
        assert_eq!(namespace[0].kind, SemanticTokenKind::Namespace);
    }

    #[test]
    fn analysis_classifies_method_self_as_a_parameter_not_a_type() {
        let text = r#"struct File { fd: i32 }

instance File {
    method &self.read(): i32 {
        return self.fd
    }
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");
        let receiver_start = text.find("self.read").expect("expected receiver");
        let receiver = identifier_starting_at(&identifiers, receiver_start)
            .expect("expected receiver semantic token");

        assert_eq!(receiver.kind, SemanticTokenKind::Parameter);
        assert_ne!(
            receiver.modifiers & SEMANTIC_READONLY_MODIFIER,
            0,
            "method receivers are readonly parameter bindings"
        );
    }

    #[test]
    fn analysis_classifies_coercion_self_as_a_readonly_parameter() {
        let text = r#"struct Text { value: &str }
coerce Text {
    pub &self as &str from self { return self.value }
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");
        let receiver_start = text.find("self as").expect("expected receiver");
        let receiver = identifier_starting_at(&identifiers, receiver_start)
            .expect("expected receiver semantic token");

        assert_eq!(receiver.kind, SemanticTokenKind::Parameter);
        assert_ne!(receiver.modifiers & SEMANTIC_READONLY_MODIFIER, 0);
    }

    #[test]
    fn analysis_classifies_drop_keyword_and_receiver_independently() {
        let text = r#"struct Token { value: i32 }

destruct Token(&+self) {
    return
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");

        let drop_keyword = identifier_starting_at(
            &identifiers,
            text.find("destruct Token")
                .expect("expected destruct declaration"),
        )
        .expect("expected drop semantic token");
        assert_eq!(drop_keyword.kind, SemanticTokenKind::Method);
        assert_ne!(drop_keyword.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);

        let receiver = identifier_starting_at(
            &identifiers,
            text.find("self) {").expect("expected drop receiver"),
        )
        .expect("expected receiver semantic token");
        assert_eq!(receiver.kind, SemanticTokenKind::Parameter);
        assert_ne!(receiver.modifiers & SEMANTIC_READONLY_MODIFIER, 0);
    }

    #[test]
    fn analysis_classifies_closure_parameters_and_capture_modes() {
        let text = r#"func main(): i32 {
    let base = 1
    var total = 2
    let owned = 3
    let transform = (&base, &+total, move owned; value: i32): i32 {
        total = total + value
        return base + total + owned
    }
    return transform(4)
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");

        for lexeme in ["base", "owned"] {
            let tokens = identifiers_for_lexeme(text, &identifiers, lexeme);
            assert!(tokens.len() >= 2, "expected capture tokens for {lexeme}");
            assert!(tokens.iter().all(|token| {
                token.kind == SemanticTokenKind::Variable
                    && token.modifiers & SEMANTIC_READONLY_MODIFIER != 0
            }));
        }

        let total_tokens = identifiers_for_lexeme(text, &identifiers, "total");
        assert!(total_tokens.len() >= 3);
        assert!(total_tokens.iter().all(|token| {
            token.kind == SemanticTokenKind::Variable
                && token.modifiers & SEMANTIC_READONLY_MODIFIER == 0
        }));

        let value_tokens = identifiers_for_lexeme(text, &identifiers, "value");
        assert!(value_tokens.len() >= 2);
        assert!(value_tokens.iter().all(|token| {
            token.kind == SemanticTokenKind::Parameter
                && token.modifiers & SEMANTIC_READONLY_MODIFIER != 0
        }));
    }

    #[test]
    fn analysis_classifies_generic_bounds_and_provenance_origins() {
        let text = r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

func read<M>(map: &M): &i32 from map where M: Lookup<i32> {
    return map.get()
}

struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self from text {
        return Text { value: text }
    }
}
"#;
        let identifiers =
            classified_identifiers_for_single_file_text(text).expect("expected semantic analysis");

        let interface_keyword = text.find("interface").unwrap();
        assert!(identifiers.iter().all(|identifier| {
            identifier.end_byte <= interface_keyword
                || identifier.start_byte >= interface_keyword + "interface".len()
        }));
        let interface_name = text.find("Lookup<V>").unwrap();
        let interface_token = identifier_starting_at(&identifiers, interface_name)
            .expect("expected interface name token");
        assert_eq!(
            &text[interface_token.start_byte..interface_token.end_byte],
            "Lookup"
        );
        assert_eq!(interface_token.kind, SemanticTokenKind::Type);

        let declaration_start = text.find("read<M>").unwrap() + "read<".len();
        let declaration = identifier_starting_at(&identifiers, declaration_start)
            .expect("expected generic parameter declaration token");
        assert_eq!(declaration.kind, SemanticTokenKind::Type);
        assert_ne!(declaration.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);

        let requirement_start = text
            .find("M: Lookup")
            .expect("expected generic requirement");
        let requirement = identifier_starting_at(&identifiers, requirement_start)
            .expect("expected generic requirement token");
        assert_eq!(requirement.kind, SemanticTokenKind::Type);
        assert_eq!(requirement.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);

        let origin_start = text.find("from map").unwrap() + "from ".len();
        let origin = identifier_starting_at(&identifiers, origin_start)
            .expect("expected provenance origin token");
        assert_eq!(origin.kind, SemanticTokenKind::Parameter);
        assert_ne!(origin.modifiers & SEMANTIC_READONLY_MODIFIER, 0);

        let literal_origin_start = text.rfind("from text").unwrap() + "from ".len();
        let literal_origin = identifier_starting_at(&identifiers, literal_origin_start)
            .expect("expected literal provenance origin token");
        assert_eq!(literal_origin.kind, SemanticTokenKind::Parameter);
        assert_ne!(literal_origin.modifiers & SEMANTIC_READONLY_MODIFIER, 0);

        let call_start = text.rfind("get()").expect("expected bound method call");
        let call =
            identifier_starting_at(&identifiers, call_start).expect("expected bound method token");
        assert_eq!(call.kind, SemanticTokenKind::Method);
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
        let text = "struct File {\n    fd: i32\n}\n\nfunc File.open(): Self {\n    return Self { fd: 1 }\n}\n\nenum Event {\n    count(value: i32)\n}\n\nfunc main(): i32 {\n    let file = File.open()\n    let event = Event.count(1)\n    return file.fd\n}\n";
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

    #[test]
    fn analysis_classifies_enum_pattern_variants_as_properties() {
        let text = r#"enum Choice {
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        for start in [
            text.find("hit(_)").expect("expected if-is hit pattern"),
            text.rfind("hit(_)").expect("expected match hit pattern"),
        ] {
            let hit = identifier_starting_at(&identifiers, start)
                .expect("expected semantic token for hit pattern variant");
            assert_eq!(hit.kind, SemanticTokenKind::Property);
        }

        let miss = identifier_starting_at(
            &identifiers,
            text.rfind("miss(_)").expect("expected match miss pattern"),
        )
        .expect("expected semantic token for miss pattern variant");
        assert_eq!(miss.kind, SemanticTokenKind::Property);

        assert!(
            identifiers_for_lexeme(text, &identifiers, "_").is_empty(),
            "payload discard should not be classified as an identifier"
        );
    }

    #[test]
    fn analysis_classifies_intrinsic_generic_requirement_words_as_keywords() {
        let text = r#"func duplicate<T>(value: T): T where copy T {
    return value
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        let copy_tokens = identifiers_for_lexeme(text, &identifiers, "copy");
        let where_tokens = identifiers_for_lexeme(text, &identifiers, "where");
        assert_eq!(copy_tokens.len(), 1);
        assert!(
            copy_tokens
                .iter()
                .all(|token| token.kind == SemanticTokenKind::Keyword)
        );
        assert_eq!(where_tokens.len(), 1);
        assert_eq!(where_tokens[0].kind, SemanticTokenKind::Keyword);
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
