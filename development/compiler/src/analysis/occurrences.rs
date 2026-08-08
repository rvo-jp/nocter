//! Canonical semantic occurrences shared by editor features.
//!
//! Resolver and typecheck retain the authoritative identities. This index only
//! joins them by source range so hover, navigation, references, and semantic
//! classification cannot develop independent target-selection rules.

use super::CompileUnitAnalysis;
use crate::ast::{
    AstFile, BindingKind, ClosureCaptureMode, ConstructMemberDecl, ImplMember, Item, TypeExpr,
};
use crate::resolve::{LocalSymbol, LocalSymbolKind, ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::TypecheckFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SemanticIdentity {
    Declaration(ByteSpan),
    Member(ByteSpan),
    Local(ByteSpan),
    GenericParameter(ByteSpan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticOccurrenceRole {
    Declaration,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticOccurrenceKind {
    Function,
    Literal,
    Method,
    Variable,
    Parameter,
    Type,
    Property,
    Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticOccurrence {
    pub(crate) focus_span: ByteSpan,
    pub(crate) identity: Option<SemanticIdentity>,
    pub(crate) role: SemanticOccurrenceRole,
    pub(crate) kind: SemanticOccurrenceKind,
    pub(crate) is_readonly: bool,
    pub(crate) contextual_type: Option<TypeExpr>,
    priority: u8,
}

impl SemanticOccurrence {
    pub(crate) fn source_target(
        &self,
        analysis: &CompileUnitAnalysis,
    ) -> Option<super::editor_targets::SourceTarget> {
        let target = match self.identity? {
            SemanticIdentity::Member(span)
            | SemanticIdentity::Local(span)
            | SemanticIdentity::GenericParameter(span) => span,
            SemanticIdentity::Declaration(declaration_span) => analysis
                .file_by_source(declaration_span.source)
                .and_then(|file| {
                    file.resolved
                        .symbols
                        .symbols()
                        .find(|symbol| symbol.declaration_span == declaration_span)
                        .map(|symbol| symbol.name_span)
                })
                .unwrap_or(declaration_span),
        };
        Some(super::editor_targets::SourceTarget::new(
            self.focus_span,
            target,
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticOccurrenceIndex {
    occurrences: Vec<SemanticOccurrence>,
}

impl SemanticOccurrenceIndex {
    pub(crate) fn new(ast: &AstFile, resolved: &ResolveOutput, facts: &TypecheckFacts) -> Self {
        let mut builder = OccurrenceBuilder {
            ast,
            source: ast.span.source,
            resolved,
            facts,
            occurrences: Vec::new(),
        };
        builder.collect();
        builder.finish()
    }

    pub(crate) fn at_offset(&self, offset: usize) -> Option<&SemanticOccurrence> {
        self.occurrences
            .iter()
            .filter(|occurrence| span_contains(occurrence.focus_span, offset))
            .min_by_key(|occurrence| {
                (
                    occurrence.focus_span.len(),
                    occurrence.priority,
                    occurrence.focus_span.start,
                )
            })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SemanticOccurrence> {
        self.occurrences.iter()
    }
}

struct OccurrenceBuilder<'a> {
    ast: &'a AstFile,
    source: SourceId,
    resolved: &'a ResolveOutput,
    facts: &'a TypecheckFacts,
    occurrences: Vec<SemanticOccurrence>,
}

impl OccurrenceBuilder<'_> {
    fn collect(&mut self) {
        self.collect_symbol_declarations();
        self.collect_callable_implementation_occurrences();
        self.collect_local_declarations_and_references();
        self.collect_resolved_references();
        self.collect_generic_parameter_declarations();
        self.collect_type_references();
        self.collect_typechecked_references();
    }

    fn collect_callable_implementation_occurrences(&mut self) {
        for item in &self.ast.items {
            match item {
                Item::Function(function) => {
                    let physical_identity = if function.owner.is_some() {
                        function.member_name_span
                    } else {
                        function.name_span
                    };
                    let Some(contract) =
                        self.resolved.callable_bodies.declaration(physical_identity)
                    else {
                        continue;
                    };
                    let (identity, kind) = if function.owner.is_some() {
                        (
                            SemanticIdentity::Member(contract),
                            SemanticOccurrenceKind::Function,
                        )
                    } else {
                        (
                            SemanticIdentity::Declaration(contract),
                            SemanticOccurrenceKind::Function,
                        )
                    };
                    self.push(
                        function.member_name_span,
                        Some(identity),
                        SemanticOccurrenceRole::Reference,
                        kind,
                        false,
                        None,
                        2,
                    );
                }
                Item::Impl(impl_) if impl_.interface_ty.is_none() => {
                    for member in &impl_.members {
                        let ImplMember::Method(method) = member else {
                            continue;
                        };
                        let Some(contract) =
                            self.resolved.callable_bodies.declaration(method.name_span)
                        else {
                            continue;
                        };
                        self.push(
                            method.name_span,
                            Some(SemanticIdentity::Member(contract)),
                            SemanticOccurrenceRole::Reference,
                            SemanticOccurrenceKind::Method,
                            false,
                            None,
                            2,
                        );
                    }
                }
                Item::Construct(construct) => {
                    for member in &construct.members {
                        let (focus, kind) = match &member.declaration {
                            ConstructMemberDecl::Function(function) => {
                                (function.member_name_span, SemanticOccurrenceKind::Function)
                            }
                            ConstructMemberDecl::Literal(literal) => {
                                (literal.shape_span, SemanticOccurrenceKind::Literal)
                            }
                        };
                        let Some(contract) = self.resolved.callable_bodies.declaration(focus)
                        else {
                            continue;
                        };
                        self.push(
                            focus,
                            Some(SemanticIdentity::Member(contract)),
                            SemanticOccurrenceRole::Reference,
                            kind,
                            false,
                            None,
                            2,
                        );
                    }
                }
                Item::Import(_)
                | Item::FromImport(_)
                | Item::Test(_)
                | Item::Primitive(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_)
                | Item::Interface(_)
                | Item::Impl(_)
                | Item::Coerce(_) => {}
            }
        }
    }

    fn finish(mut self) -> SemanticOccurrenceIndex {
        self.occurrences.sort_by_key(|occurrence| {
            (
                occurrence.focus_span.start,
                occurrence.focus_span.end,
                occurrence.priority,
            )
        });
        self.occurrences.dedup_by(|left, right| {
            left.focus_span == right.focus_span
                && left.identity == right.identity
                && left.role == right.role
                && left.kind == right.kind
        });
        SemanticOccurrenceIndex {
            occurrences: self.occurrences,
        }
    }

    fn collect_symbol_declarations(&mut self) {
        for symbol in self.resolved.symbols.symbols() {
            if symbol.name_span.source != self.source {
                continue;
            }
            let role = if symbol.declaration_span.source == self.source {
                SemanticOccurrenceRole::Declaration
            } else {
                SemanticOccurrenceRole::Reference
            };
            match &symbol.kind {
                SymbolKind::Function(_) | SymbolKind::Primitive(_) => self.push(
                    symbol.name_span,
                    Some(SemanticIdentity::Declaration(symbol.declaration_span)),
                    role,
                    SemanticOccurrenceKind::Function,
                    false,
                    None,
                    3,
                ),
                SymbolKind::Type(type_symbol) => {
                    self.push(
                        symbol.name_span,
                        Some(SemanticIdentity::Declaration(symbol.declaration_span)),
                        role,
                        SemanticOccurrenceKind::Type,
                        false,
                        None,
                        3,
                    );
                    if role == SemanticOccurrenceRole::Declaration {
                        for field in &type_symbol.fields {
                            self.push_member_declaration(
                                field.name_span,
                                SemanticOccurrenceKind::Property,
                            );
                        }
                        for variant in &type_symbol.variants {
                            self.push_member_declaration(
                                variant.name_span,
                                SemanticOccurrenceKind::Property,
                            );
                        }
                        for function in &type_symbol.associated_functions {
                            self.push_member_declaration(
                                function.name_span,
                                SemanticOccurrenceKind::Function,
                            );
                        }
                        for method in &type_symbol.methods {
                            self.push_member_declaration(
                                method.name_span,
                                SemanticOccurrenceKind::Method,
                            );
                        }
                        for literal in &type_symbol.literals {
                            self.push_member_declaration(
                                literal.shape_span,
                                SemanticOccurrenceKind::Literal,
                            );
                        }
                        if let Some(drop_) = &type_symbol.drop_member {
                            self.push_member_declaration(
                                drop_.name_span,
                                SemanticOccurrenceKind::Method,
                            );
                        }
                    }
                }
                SymbolKind::Imported(imported) => {
                    if matches!(imported.kind, crate::resolve::ImportedSymbolKind::Namespace) {
                        self.push(
                            symbol.name_span,
                            None,
                            SemanticOccurrenceRole::Reference,
                            SemanticOccurrenceKind::Namespace,
                            false,
                            None,
                            3,
                        );
                    }
                }
            }
        }
    }

    fn push_member_declaration(&mut self, span: ByteSpan, kind: SemanticOccurrenceKind) {
        if span.source == self.source {
            self.push(
                span,
                Some(SemanticIdentity::Member(span)),
                SemanticOccurrenceRole::Declaration,
                kind,
                false,
                None,
                2,
            );
        }
    }

    fn collect_local_declarations_and_references(&mut self) {
        for symbol in self.resolved.local_symbols() {
            self.push_local(
                symbol.name_span,
                symbol,
                SemanticOccurrenceRole::Declaration,
            );
        }
        for (span, symbol) in self.resolved.local_symbol_identifier_references() {
            self.push_local(span, symbol, SemanticOccurrenceRole::Reference);
        }
    }

    fn push_local(&mut self, span: ByteSpan, symbol: &LocalSymbol, role: SemanticOccurrenceRole) {
        let kind = match symbol.kind {
            LocalSymbolKind::Parameter => SemanticOccurrenceKind::Parameter,
            LocalSymbolKind::Binding(_)
            | LocalSymbolKind::Region
            | LocalSymbolKind::LiteralCapture
            | LocalSymbolKind::ClosureCapture(_)
            | LocalSymbolKind::PatternPayload
            | LocalSymbolKind::CatchError
            | LocalSymbolKind::ForRange
            | LocalSymbolKind::CollectionFor
            | LocalSymbolKind::LiteralPackFor => SemanticOccurrenceKind::Variable,
        };
        // A closure capture spelling simultaneously declares the inner capture
        // and references the outer binding. Cursor queries select the outer
        // reference at that site; references inside the body select the capture.
        let priority = if role == SemanticOccurrenceRole::Declaration
            && matches!(symbol.kind, LocalSymbolKind::ClosureCapture(_))
        {
            3
        } else {
            2
        };
        self.push(
            span,
            Some(SemanticIdentity::Local(symbol.name_span)),
            role,
            kind,
            local_is_readonly(symbol, span, self.facts),
            None,
            priority,
        );
    }

    fn collect_resolved_references(&mut self) {
        for (span, symbol) in self.resolved.symbol_identifier_references() {
            let kind = match symbol.kind {
                SymbolKind::Function(_) | SymbolKind::Primitive(_) => {
                    SemanticOccurrenceKind::Function
                }
                SymbolKind::Type(_) => SemanticOccurrenceKind::Type,
                SymbolKind::Imported(_) => continue,
            };
            self.push(
                span,
                Some(SemanticIdentity::Declaration(symbol.declaration_span)),
                SemanticOccurrenceRole::Reference,
                kind,
                false,
                None,
                2,
            );
        }
    }

    fn collect_type_references(&mut self) {
        for occurrence in self.facts.type_occurrences() {
            self.push(
                occurrence.focus_span,
                occurrence.target.map(|target| match target {
                    crate::typecheck::TypeOccurrenceTarget::Declaration(span) => {
                        SemanticIdentity::Declaration(span)
                    }
                    crate::typecheck::TypeOccurrenceTarget::GenericParameter(span) => {
                        SemanticIdentity::GenericParameter(span)
                    }
                }),
                SemanticOccurrenceRole::Reference,
                SemanticOccurrenceKind::Type,
                false,
                Some(occurrence.contextual_type.clone()),
                1,
            );
        }
    }

    fn collect_generic_parameter_declarations(&mut self) {
        let spans = self
            .facts
            .generic_parameter_declarations()
            .map(|parameter| parameter.span)
            .collect::<Vec<_>>();
        for span in spans {
            self.push(
                span,
                Some(SemanticIdentity::GenericParameter(span)),
                SemanticOccurrenceRole::Declaration,
                SemanticOccurrenceKind::Type,
                false,
                None,
                1,
            );
        }
    }

    fn collect_typechecked_references(&mut self) {
        let function_calls = self
            .facts
            .function_call_target_spans()
            .filter_map(|span| {
                self.facts
                    .function_call_target(span)
                    .map(|target| (span, target))
            })
            .collect::<Vec<_>>();
        for (span, target) in function_calls {
            self.push_reference(span, target, SemanticOccurrenceKind::Function, false, false);
        }

        let associated_functions = self
            .facts
            .associated_function_target_spans()
            .filter_map(|span| {
                self.facts
                    .associated_function_target(span)
                    .map(|target| (span, target))
            })
            .collect::<Vec<_>>();
        for (span, target) in associated_functions {
            self.push_reference(span, target, SemanticOccurrenceKind::Function, false, true);
        }

        let methods = self
            .facts
            .method_call_spans()
            .filter_map(|span| {
                self.facts
                    .method_call_target(span)
                    .map(|target| (span, target))
            })
            .collect::<Vec<_>>();
        for (span, target) in methods {
            self.push_reference(span, target, SemanticOccurrenceKind::Method, false, true);
        }

        let fields = self
            .facts
            .field_target_spans()
            .filter_map(|span| {
                self.facts.field_target(span).map(|target| {
                    (
                        span,
                        target,
                        self.facts.field_is_readonly(span) == Some(true),
                    )
                })
            })
            .collect::<Vec<_>>();
        for (span, target, readonly) in fields {
            self.push_reference(
                span,
                target,
                SemanticOccurrenceKind::Property,
                readonly,
                true,
            );
        }

        let variants = self
            .facts
            .enum_variant_target_spans()
            .filter_map(|span| {
                self.facts
                    .enum_variant_target(span)
                    .map(|target| (span, target))
            })
            .collect::<Vec<_>>();
        for (span, target) in variants {
            self.push_reference(span, target, SemanticOccurrenceKind::Property, false, true);
        }
    }

    fn push_reference(
        &mut self,
        span: ByteSpan,
        target: ByteSpan,
        kind: SemanticOccurrenceKind,
        is_readonly: bool,
        is_member: bool,
    ) {
        let identity = if is_member {
            SemanticIdentity::Member(target)
        } else {
            SemanticIdentity::Declaration(target)
        };
        self.push(
            span,
            Some(identity),
            SemanticOccurrenceRole::Reference,
            kind,
            is_readonly,
            None,
            0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        focus_span: ByteSpan,
        identity: Option<SemanticIdentity>,
        role: SemanticOccurrenceRole,
        kind: SemanticOccurrenceKind,
        is_readonly: bool,
        contextual_type: Option<TypeExpr>,
        priority: u8,
    ) {
        if focus_span.source == self.source {
            self.occurrences.push(SemanticOccurrence {
                focus_span,
                identity,
                role,
                kind,
                is_readonly,
                contextual_type,
                priority,
            });
        }
    }
}

fn local_is_readonly(symbol: &LocalSymbol, span: ByteSpan, facts: &TypecheckFacts) -> bool {
    match symbol.kind {
        LocalSymbolKind::Parameter
        | LocalSymbolKind::Binding(BindingKind::Let)
        | LocalSymbolKind::Region
        | LocalSymbolKind::LiteralCapture
        | LocalSymbolKind::ClosureCapture(ClosureCaptureMode::ReadonlyBorrow)
        | LocalSymbolKind::ClosureCapture(ClosureCaptureMode::Move) => true,
        LocalSymbolKind::ClosureCapture(ClosureCaptureMode::ReadwriteBorrow)
        | LocalSymbolKind::Binding(BindingKind::Var) => false,
        LocalSymbolKind::PatternPayload
        | LocalSymbolKind::CatchError
        | LocalSymbolKind::ForRange
        | LocalSymbolKind::CollectionFor
        | LocalSymbolKind::LiteralPackFor => facts.binding_is_readonly(span) == Some(true),
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;
    use crate::ast::canonical_type_expr;

    #[test]
    fn impl_bounds_and_targets_share_one_interface_identity() {
        let text = r#"interface ExactSizeIterator<T> {}

struct Indexed<T> { value: T }
struct EnumerateIter<T, I> { source: I }

impl<T, I: ExactSizeIterator<T>> ExactSizeIterator<Indexed<T>> for EnumerateIter<T, I> {}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let declaration_offset = text.find("ExactSizeIterator<T> {}").unwrap();
        let bound_offset = text.find("I: ExactSizeIterator").unwrap() + "I: ".len();
        let target_offset = text.rfind(">> ExactSizeIterator").unwrap() + ">> ".len();

        let declaration = file.occurrences.at_offset(declaration_offset).unwrap();
        let bound = file.occurrences.at_offset(bound_offset).unwrap();
        let target = file.occurrences.at_offset(target_offset).unwrap();

        assert_eq!(declaration.role, SemanticOccurrenceRole::Declaration);
        assert_eq!(bound.role, SemanticOccurrenceRole::Reference);
        assert_eq!(target.role, SemanticOccurrenceRole::Reference);
        assert_eq!(bound.identity, declaration.identity);
        assert_eq!(target.identity, declaration.identity);
        assert_eq!(bound.kind, SemanticOccurrenceKind::Type);
        assert_eq!(target.kind, SemanticOccurrenceKind::Type);
        assert_eq!(
            bound.contextual_type.as_ref().map(canonical_type_expr),
            Some("ExactSizeIterator<T>".to_string())
        );
        assert_eq!(
            target.contextual_type.as_ref().map(canonical_type_expr),
            Some("ExactSizeIterator<Indexed<T>>".to_string())
        );
    }

    #[test]
    fn explicit_closure_types_are_semantic_type_occurrences() {
        let text = r#"struct Input { value: i32 }
struct Output { value: i32 }

func main(): i32 {
    let transform = (value: Input): Output { Output { value: value.value } }
    return 0
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");

        for offset in [
            text.find("value: Input").unwrap() + "value: ".len(),
            text.find("): Output").unwrap() + "): ".len(),
        ] {
            let occurrence = file
                .occurrences
                .at_offset(offset)
                .expect("expected closure type occurrence");
            assert_eq!(occurrence.kind, SemanticOccurrenceKind::Type);
            assert_eq!(occurrence.role, SemanticOccurrenceRole::Reference);
            assert!(occurrence.identity.is_some());
        }
    }

    #[test]
    fn method_generic_parameters_have_declaration_and_reference_identity() {
        let text = r#"interface Iterator<T> {}

interface Transform<T> {
    pub method &self.map<U: Iterator<T>>(value: U): T
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let declaration_offset = text.find("U: Iterator").unwrap();
        let reference_offset = text.find("value: U").unwrap() + "value: ".len();
        let declaration = file.occurrences.at_offset(declaration_offset).unwrap();
        let reference = file.occurrences.at_offset(reference_offset).unwrap();

        assert_eq!(
            declaration.identity,
            Some(SemanticIdentity::GenericParameter(declaration.focus_span))
        );
        assert_eq!(reference.identity, declaration.identity);
        assert_eq!(declaration.role, SemanticOccurrenceRole::Declaration);
        assert_eq!(reference.role, SemanticOccurrenceRole::Reference);
    }
}
