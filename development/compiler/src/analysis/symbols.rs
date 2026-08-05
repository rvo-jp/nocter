//! Document outline symbols derived from the parsed AST.

use super::single_file::parse_single_file_text;
use crate::ast::{AstFile, ImplDecl, ImplMember, Item, MethodDecl};
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
        Item::Impl(impl_) => Some(document_symbol(
            impl_document_symbol_name(text, impl_),
            DocumentSymbolKind::Class,
            impl_.span,
            impl_document_symbol_selection_span(impl_),
            impl_
                .members
                .iter()
                .map(impl_member_document_symbol)
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
    }
}

fn impl_member_document_symbol(member: &ImplMember) -> DocumentSymbolInfo {
    match member {
        ImplMember::Method(method) => method_document_symbol(method),
        ImplMember::Drop(drop_) => document_symbol(
            "drop".to_string(),
            DocumentSymbolKind::Method,
            drop_.span,
            drop_.name_span,
            Vec::new(),
        ),
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

fn impl_document_symbol_name(text: &str, impl_: &ImplDecl) -> String {
    if let Some(interface_ty) = &impl_.interface_ty {
        return format!(
            "impl {} for {}",
            source_fragment(text, interface_ty.span()),
            source_fragment(text, impl_.target_ty.span())
        );
    }

    format!("impl {}", source_fragment(text, impl_.target_ty.span()))
}

fn impl_document_symbol_selection_span(impl_: &ImplDecl) -> ByteSpan {
    impl_
        .interface_ty
        .as_ref()
        .map_or(impl_.target_ty.span(), |interface_ty| interface_ty.span())
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
    fn drop_document_symbol_selects_the_declaration_keyword() {
        let text = r#"struct Token { value: i32 }

impl Token {
    drop &+self {
        return
    }
}
"#;
        let symbols = document_symbols_for_text(text).expect("expected document symbols");
        let drop_symbol = &symbols[1].children[0];

        assert_eq!(drop_symbol.name, "drop");
        assert_eq!(drop_symbol.kind, DocumentSymbolKind::Method);
        assert_eq!(
            &text[drop_symbol.selection_span.start..drop_symbol.selection_span.end],
            "drop"
        );
    }

    #[test]
    fn document_symbols_survive_an_unclosed_member_body() {
        let text = r#"struct Token { value: i32 }

impl Token {
    drop &+self {
        return
"#;
        let symbols = document_symbols_for_text(text).expect("expected recovered document symbols");

        assert_eq!(symbols[0].name, "Token");
        assert_eq!(symbols[1].children[0].name, "drop");
        assert_eq!(
            &text[symbols[1].children[0].selection_span.start
                ..symbols[1].children[0].selection_span.end],
            "drop"
        );
    }
}
