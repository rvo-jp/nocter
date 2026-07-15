use super::documents::OpenDocument;
use super::protocol::range_for_byte_span;
use crate::ast::{ImplMember, Item, MethodDecl};
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

const LSP_SYMBOL_KIND_CLASS: u8 = 5;
const LSP_SYMBOL_KIND_METHOD: u8 = 6;
pub(super) const LSP_SYMBOL_KIND_FIELD: u8 = 8;
const LSP_SYMBOL_KIND_ENUM: u8 = 10;
const LSP_SYMBOL_KIND_INTERFACE: u8 = 11;
pub(super) const LSP_SYMBOL_KIND_FUNCTION: u8 = 12;
pub(super) const LSP_SYMBOL_KIND_ENUM_MEMBER: u8 = 22;
pub(super) const LSP_SYMBOL_KIND_STRUCT: u8 = 23;

pub(super) fn document_symbols_for_document(document: &OpenDocument) -> Option<Vec<Value>> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;

    Some(
        ast.items
            .iter()
            .filter_map(|item| item_document_symbol(&document.text, item))
            .collect(),
    )
}

fn item_document_symbol(text: &str, item: &Item) -> Option<Value> {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => None,
        Item::Function(function) => Some(document_symbol(
            text,
            &function.name,
            LSP_SYMBOL_KIND_FUNCTION,
            function.span,
            function.name_span,
            Vec::new(),
        )),
        Item::Primitive(primitive) => Some(document_symbol(
            text,
            &primitive.name,
            LSP_SYMBOL_KIND_FUNCTION,
            primitive.span,
            primitive.name_span,
            Vec::new(),
        )),
        Item::TypeAlias(alias) => Some(document_symbol(
            text,
            &alias.name,
            LSP_SYMBOL_KIND_CLASS,
            alias.span,
            alias.name_span,
            Vec::new(),
        )),
        Item::Struct(struct_) => Some(document_symbol(
            text,
            &struct_.name,
            LSP_SYMBOL_KIND_STRUCT,
            struct_.span,
            struct_.name_span,
            struct_
                .fields
                .iter()
                .map(|field| {
                    document_symbol(
                        text,
                        &field.name,
                        LSP_SYMBOL_KIND_FIELD,
                        field.span,
                        field.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Enum(enum_) => Some(document_symbol(
            text,
            &enum_.name,
            LSP_SYMBOL_KIND_ENUM,
            enum_.span,
            enum_.name_span,
            enum_
                .variants
                .iter()
                .map(|variant| {
                    document_symbol(
                        text,
                        &variant.name,
                        LSP_SYMBOL_KIND_ENUM_MEMBER,
                        variant.span,
                        variant.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Trait(trait_) => Some(document_symbol(
            text,
            &trait_.name,
            LSP_SYMBOL_KIND_INTERFACE,
            trait_.span,
            trait_.name_span,
            trait_
                .methods
                .iter()
                .map(|method| method_document_symbol(text, method))
                .collect(),
        )),
        Item::Impl(impl_) => Some(document_symbol(
            text,
            &format!("impl {}", source_fragment(text, impl_.target_ty.span())),
            LSP_SYMBOL_KIND_CLASS,
            impl_.span,
            impl_.target_ty.span(),
            impl_
                .members
                .iter()
                .map(|member| impl_member_document_symbol(text, member))
                .collect(),
        )),
    }
}

fn impl_member_document_symbol(text: &str, member: &ImplMember) -> Value {
    match member {
        ImplMember::Function(function) => document_symbol(
            text,
            &function.member_name,
            LSP_SYMBOL_KIND_FUNCTION,
            function.span,
            function.member_name_span,
            Vec::new(),
        ),
        ImplMember::Method(method) => method_document_symbol(text, method),
        ImplMember::Drop(drop_) => document_symbol(
            text,
            "drop",
            LSP_SYMBOL_KIND_METHOD,
            drop_.span,
            drop_.binding.name_span,
            Vec::new(),
        ),
    }
}

fn method_document_symbol(text: &str, method: &MethodDecl) -> Value {
    document_symbol(
        text,
        &method.name,
        LSP_SYMBOL_KIND_METHOD,
        method.span,
        method.name_span,
        Vec::new(),
    )
}

fn document_symbol(
    text: &str,
    name: &str,
    kind: u8,
    range_span: ByteSpan,
    selection_span: ByteSpan,
    children: Vec<Value>,
) -> Value {
    let mut symbol = json!({
        "name": name,
        "kind": kind,
        "range": range_for_byte_span(text, range_span),
        "selectionRange": range_for_byte_span(text, selection_span)
    });

    if !children.is_empty()
        && let Some(object) = symbol.as_object_mut()
    {
        object.insert("children".to_string(), Value::Array(children));
    }

    symbol
}

fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}
