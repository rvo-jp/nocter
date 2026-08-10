//! Document outline symbols derived from the parsed AST.

use super::single_file::parse_single_file_text;
use crate::ast::{AstFile, ConformanceMember, Item, MethodDecl};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentSymbolKind {
    Class,
    Method,
    Field,
    Enum,
    Interface,
    Function,
    EnumMember,
    Struct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocumentSymbolInfo {
    pub(crate) name: String,
    pub(crate) kind: DocumentSymbolKind,
    pub(crate) range_span: ByteSpan,
    pub(crate) selection_span: ByteSpan,
    pub(crate) children: Vec<DocumentSymbolInfo>,
}

pub(crate) fn document_symbols_for_text(text: &str) -> Option<Vec<DocumentSymbolInfo>> {
    let parsed = parse_single_file_text("document-symbols.nct", text).or_else(|| {
        let recovered = super::delimiter_recovery::block_recovery_text(text, text.len())?;
        parse_single_file_text("document-symbols.nct", &recovered)
    })?;

    Some(document_symbols_for_ast(text, &parsed.ast))
}

pub(crate) fn document_symbols_for_ast(text: &str, ast: &AstFile) -> Vec<DocumentSymbolInfo> {
    ast.items
        .iter()
        .filter_map(|item| item_document_symbol(text, item))
        .collect()
}

fn item_document_symbol(text: &str, item: &Item) -> Option<DocumentSymbolInfo> {
    match item {
        Item::Import(_) | Item::FromImport(_) => None,
        Item::Function(function) => Some(document_symbol(
            function.name.clone(),
            DocumentSymbolKind::Function,
            function.span,
            function.name_span,
            Vec::new(),
        )),
        Item::Test(test) => Some(document_symbol(
            test.name.clone(),
            DocumentSymbolKind::Function,
            test.span,
            test.name_span,
            Vec::new(),
        )),
        Item::Primitive(primitive) => Some(document_symbol(
            primitive.name.clone(),
            DocumentSymbolKind::Function,
            primitive.span,
            primitive.name_span,
            Vec::new(),
        )),
        Item::TypeAlias(alias) => Some(document_symbol(
            alias.name.clone(),
            DocumentSymbolKind::Class,
            alias.span,
            alias.name_span,
            Vec::new(),
        )),
        Item::Struct(struct_) => Some(document_symbol(
            struct_.name.clone(),
            DocumentSymbolKind::Struct,
            struct_.span,
            struct_.name_span,
            struct_
                .fields
                .iter()
                .map(|field| {
                    document_symbol(
                        field.name.clone(),
                        DocumentSymbolKind::Field,
                        field.span,
                        field.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Enum(enum_) => Some(document_symbol(
            enum_.name.clone(),
            DocumentSymbolKind::Enum,
            enum_.span,
            enum_.name_span,
            enum_
                .variants
                .iter()
                .map(|variant| {
                    document_symbol(
                        variant.name.clone(),
                        DocumentSymbolKind::EnumMember,
                        variant.span,
                        variant.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Interface(interface) => Some(document_symbol(
            interface.name.clone(),
            DocumentSymbolKind::Interface,
            interface.span,
            interface.name_span,
            interface
                .methods
                .iter()
                .map(method_document_symbol)
                .collect(),
        )),
        Item::Instance(instance) => Some(document_symbol(
            format!(
                "instance {}",
                source_fragment(text, instance.target_ty.span())
            ),
            DocumentSymbolKind::Class,
            instance.span,
            instance.target_ty.span(),
            instance
                .methods
                .iter()
                .map(method_document_symbol)
                .collect(),
        )),
        Item::Destruct(destruct) => Some(document_symbol(
            format!(
                "destruct {}",
                source_fragment(text, destruct.target_ty.span())
            ),
            DocumentSymbolKind::Method,
            destruct.span,
            destruct.keyword_span,
            Vec::new(),
        )),
        Item::Conformance(conformance) => Some(document_symbol(
            format!(
                "conform {} for {}",
                source_fragment(text, conformance.interface_ty.span()),
                source_fragment(text, conformance.target_ty.span())
            ),
            DocumentSymbolKind::Class,
            conformance.span,
            conformance.interface_ty.span(),
            conformance
                .members
                .iter()
                .map(conformance_member_document_symbol)
                .collect(),
        )),
        Item::Construct(construct) => Some(document_symbol(
            format!(
                "construct {}",
                source_fragment(text, construct.target.span())
            ),
            DocumentSymbolKind::Class,
            construct.span,
            construct.target.span(),
            construct
                .members
                .iter()
                .map(|member| match &member.declaration {
                    crate::ast::ConstructMemberDecl::Function(function) => document_symbol(
                        function.member_name.clone(),
                        DocumentSymbolKind::Function,
                        member.span,
                        function.member_name_span,
                        Vec::new(),
                    ),
                    crate::ast::ConstructMemberDecl::Literal(literal) => document_symbol(
                        match literal.shape {
                            crate::ast::LiteralShape::Sequence => "literal []".to_string(),
                            crate::ast::LiteralShape::String => "literal \"\"".to_string(),
                        },
                        DocumentSymbolKind::Function,
                        member.span,
                        literal.shape_span,
                        Vec::new(),
                    ),
                })
                .collect(),
        )),
        Item::Coerce(coerce) => Some(document_symbol(
            format!("coerce {}", source_fragment(text, coerce.target.span())),
            DocumentSymbolKind::Class,
            coerce.span,
            coerce.target.span(),
            coerce
                .entries
                .iter()
                .map(|entry| {
                    document_symbol(
                        format!(
                            "{}self as {}",
                            entry.receiver.mode.source_prefix(),
                            source_fragment(text, entry.target.span())
                        ),
                        DocumentSymbolKind::Method,
                        entry.span,
                        entry.as_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
    }
}

fn conformance_member_document_symbol(member: &ConformanceMember) -> DocumentSymbolInfo {
    match member {
        ConformanceMember::AssociatedType(binding) => document_symbol(
            binding.name.clone(),
            DocumentSymbolKind::Class,
            binding.span,
            binding.name_span,
            Vec::new(),
        ),
        ConformanceMember::Method(method) => method_document_symbol(method),
    }
}

fn method_document_symbol(method: &MethodDecl) -> DocumentSymbolInfo {
    document_symbol(
        method.name.clone(),
        DocumentSymbolKind::Method,
        method.span,
        method.name_span,
        Vec::new(),
    )
}

fn document_symbol(
    name: String,
    kind: DocumentSymbolKind,
    range_span: ByteSpan,
    selection_span: ByteSpan,
    children: Vec<DocumentSymbolInfo>,
) -> DocumentSymbolInfo {
    DocumentSymbolInfo {
        name,
        kind,
        range_span,
        selection_span,
        children,
    }
}

fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_symbols_include_nested_struct_fields() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc main(): i32 {\n    return 0\n}\n";
        let symbols = document_symbols_for_text(text).expect("expected document symbols");

        assert_eq!(symbols[0].name, "File");
        assert_eq!(symbols[0].kind, DocumentSymbolKind::Struct);
        assert_eq!(symbols[0].children[0].name, "fd");
        assert_eq!(symbols[0].children[0].kind, DocumentSymbolKind::Field);
        assert_eq!(symbols[1].name, "main");
        assert_eq!(symbols[1].kind, DocumentSymbolKind::Function);
    }

    #[test]
    fn native_tests_are_function_like_document_symbols_with_exact_name_selection() {
        let text = "test pushes { return }\n";
        let symbols = document_symbols_for_text(text).expect("document symbols");
        assert_eq!(symbols[0].name, "pushes");
        assert_eq!(symbols[0].kind, DocumentSymbolKind::Function);
        assert_eq!(
            &text[symbols[0].selection_span.start..symbols[0].selection_span.end],
            "pushes"
        );
    }

    #[test]
    fn drop_document_symbol_selects_the_declaration_keyword() {
        let text = r#"struct Token { value: i32 }

destruct Token(&+self) {
    return
}
"#;
        let symbols = document_symbols_for_text(text).expect("expected document symbols");
        let drop_symbol = &symbols[1];

        assert_eq!(drop_symbol.name, "destruct Token");
        assert_eq!(drop_symbol.kind, DocumentSymbolKind::Method);
        assert_eq!(
            &text[drop_symbol.selection_span.start..drop_symbol.selection_span.end],
            "destruct"
        );
    }

    #[test]
    fn document_symbols_survive_an_unclosed_member_body() {
        let text = r#"struct Token { value: i32 }

destruct Token(&+self) {
    return
"#;
        let symbols = document_symbols_for_text(text).expect("expected recovered document symbols");

        assert_eq!(symbols[0].name, "Token");
        assert_eq!(symbols[1].name, "destruct Token");
        assert_eq!(
            &text[symbols[1].selection_span.start..symbols[1].selection_span.end],
            "destruct"
        );
    }
}
