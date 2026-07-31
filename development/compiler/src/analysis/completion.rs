//! Completion candidates derived from lexical keywords and resolver symbols.

use super::FileAnalysis;
use super::completion_recovery::completion_recovery_text;
use super::scoped_imports::visible_scoped_import_spans_at_offset;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::ast::{
    AstFile, Block, Expr, IfIsStmt, ImplMember, Item, MemberExpr, Stmt, StructLiteralExpr,
    SwitchArm, SwitchStmt, TypeExpr,
};
use crate::lexer::KEYWORD_LEXEMES;
use crate::resolve::{
    AssociatedFunctionSignature, EnumVariantSignature, MethodSignature, ResolveOutput,
    StructFieldSignature, Symbol, SymbolKind, TypeSymbol, TypeSymbolKind,
};
use crate::source::ByteSpan;
use crate::typecheck::{TypecheckFacts, collect_typecheck_facts};
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionItemKind {
    Function,
    Method,
    Class,
    Interface,
    Module,
    Enum,
    EnumMember,
    Field,
    Keyword,
    Struct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionItemInfo {
    pub(crate) label: String,
    pub(crate) kind: CompletionItemKind,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionContext<'a> {
    EnumPatternMembers(&'a str),
    MemberAccess {
        owner_name: &'a str,
        owner_span: ByteSpan,
    },
    StructLiteralFields {
        literal: &'a StructLiteralExpr,
        offset: usize,
    },
}

#[cfg(test)]
pub(crate) fn completion_items_for_file_analysis(file: &FileAnalysis) -> Vec<CompletionItemInfo> {
    completion_items_for_resolved_symbols(&file.resolved, HashSet::new())
}

pub(crate) fn completion_items_for_file_analysis_at_offset(
    file: &FileAnalysis,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    if let Some(items) =
        contextual_completion_items(&file.ast, &file.resolved, &file.typecheck_facts, offset)
    {
        return items;
    }

    completion_items_for_resolved_symbols(
        &file.resolved,
        visible_scoped_import_spans_at_offset(&file.ast, offset),
    )
}

pub(crate) fn completion_items_for_text_at_offset(
    text: &str,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let (completion_text, parsed) = match parse_single_file_text("completion.nct", text) {
        Some(parsed) => (Cow::Borrowed(text), parsed),
        None => {
            let completion_text = completion_recovery_text(text, offset)?;
            let parsed = parse_single_file_text("completion.nct", &completion_text)?;
            (Cow::Owned(completion_text), parsed)
        }
    };
    let resolved = resolve_single_file_ast(
        "completion.nct",
        completion_text.as_ref(),
        parsed.source,
        &parsed.ast,
    );
    let facts = collect_typecheck_facts(&parsed.ast, &resolved);

    if let Some(items) = contextual_completion_items(&parsed.ast, &resolved, &facts, offset) {
        return Some(items);
    }

    Some(completion_items_for_resolved_symbols(
        &resolved,
        visible_scoped_import_spans_at_offset(&parsed.ast, offset),
    ))
}

pub(crate) fn keyword_completion_items() -> Vec<CompletionItemInfo> {
    KEYWORD_LEXEMES
        .iter()
        .map(|keyword| CompletionItemInfo {
            label: (*keyword).to_string(),
            kind: CompletionItemKind::Keyword,
            detail: Some("keyword".to_string()),
        })
        .collect()
}

fn completion_items_for_resolved_symbols(
    resolved: &ResolveOutput,
    visible_hidden_symbol_spans: HashSet<ByteSpan>,
) -> Vec<CompletionItemInfo> {
    let mut items = keyword_completion_items();
    let mut seen = KEYWORD_LEXEMES
        .iter()
        .map(|keyword| (*keyword).to_string())
        .collect::<HashSet<_>>();

    let mut symbols = resolved
        .symbols
        .symbols()
        .filter(|symbol| {
            !symbol.is_hidden || visible_hidden_symbol_spans.contains(&symbol.name_span)
        })
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.name.cmp(&right.name));

    for symbol in symbols {
        if !seen.insert(symbol.name.clone()) {
            continue;
        }
        items.push(CompletionItemInfo {
            label: symbol.name.clone(),
            kind: completion_kind_for_symbol(symbol),
            detail: Some(symbol_detail(symbol)),
        });
    }

    items
}

fn completion_kind_for_symbol(symbol: &Symbol) -> CompletionItemKind {
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => CompletionItemKind::Function,
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => CompletionItemKind::Class,
            TypeSymbolKind::Struct => CompletionItemKind::Struct,
            TypeSymbolKind::Enum => CompletionItemKind::Enum,
            TypeSymbolKind::Interface => CompletionItemKind::Interface,
        },
        SymbolKind::Imported(_) => CompletionItemKind::Module,
    }
}

fn symbol_detail(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(_) => "function".to_string(),
        SymbolKind::Primitive(_) => "primitive".to_string(),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => "type".to_string(),
            TypeSymbolKind::Struct => "struct".to_string(),
            TypeSymbolKind::Enum => "enum".to_string(),
            TypeSymbolKind::Interface => "interface".to_string(),
        },
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

fn contextual_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    match completion_context_at_offset(ast, offset)? {
        CompletionContext::EnumPatternMembers(enum_name) => Some(
            resolved
                .type_symbol_by_name(enum_name)
                .map(enum_variant_completion_items)
                .unwrap_or_default(),
        ),
        CompletionContext::MemberAccess {
            owner_name,
            owner_span,
        } => Some(member_completion_items(
            resolved, facts, owner_name, owner_span,
        )),
        CompletionContext::StructLiteralFields { literal, offset } => Some(
            struct_literal_field_completion_items(resolved, literal, offset),
        ),
    }
}

fn member_completion_items(
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    owner_name: &str,
    owner_span: ByteSpan,
) -> Vec<CompletionItemInfo> {
    if let Some(symbol) = resolved.type_symbol_by_name(owner_name) {
        return type_member_completion_items(symbol);
    }

    let Some(owner_ty) = facts.expression_type_expr(owner_span) else {
        return Vec::new();
    };
    let Some(symbol) = type_symbol_for_value_member_completion(resolved, owner_ty) else {
        return Vec::new();
    };
    value_member_completion_items(symbol)
}

fn type_member_completion_items(symbol: &TypeSymbol) -> Vec<CompletionItemInfo> {
    let mut items = Vec::new();
    if symbol.kind == TypeSymbolKind::Enum {
        items.extend(enum_variant_completion_items(symbol));
    }
    items.extend(
        symbol
            .associated_functions
            .iter()
            .filter(|function| function.is_accessible)
            .map(associated_function_completion_item),
    );
    items
}

fn value_member_completion_items(symbol: &TypeSymbol) -> Vec<CompletionItemInfo> {
    let mut items = Vec::new();
    items.extend(
        symbol
            .fields
            .iter()
            .filter(|field| field.is_accessible)
            .map(struct_field_completion_item),
    );
    items.extend(
        symbol
            .methods
            .iter()
            .filter(|method| method.is_accessible)
            .map(method_completion_item),
    );
    items
}

fn struct_literal_field_completion_items(
    resolved: &ResolveOutput,
    literal: &StructLiteralExpr,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    let Some(symbol) = type_symbol_for_value_member_completion(resolved, &literal.ty) else {
        return Vec::new();
    };
    let used_fields = literal
        .fields
        .iter()
        .filter(|field| !span_contains(field.name_span, offset))
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();

    symbol
        .fields
        .iter()
        .filter(|field| field.is_accessible && !used_fields.contains(field.name.as_str()))
        .map(struct_field_completion_item)
        .collect()
}

fn enum_variant_completion_items(symbol: &TypeSymbol) -> Vec<CompletionItemInfo> {
    symbol
        .variants
        .iter()
        .map(enum_variant_completion_item)
        .collect()
}

fn enum_variant_completion_item(variant: &EnumVariantSignature) -> CompletionItemInfo {
    CompletionItemInfo {
        label: variant.name.clone(),
        kind: CompletionItemKind::EnumMember,
        detail: Some("enum variant".to_string()),
    }
}

fn associated_function_completion_item(
    function: &AssociatedFunctionSignature,
) -> CompletionItemInfo {
    CompletionItemInfo {
        label: function.name.clone(),
        kind: CompletionItemKind::Function,
        detail: Some("associated function".to_string()),
    }
}

fn struct_field_completion_item(field: &StructFieldSignature) -> CompletionItemInfo {
    CompletionItemInfo {
        label: field.name.clone(),
        kind: CompletionItemKind::Field,
        detail: Some("field".to_string()),
    }
}

fn method_completion_item(method: &MethodSignature) -> CompletionItemInfo {
    CompletionItemInfo {
        label: method.name.clone(),
        kind: CompletionItemKind::Method,
        detail: Some("method".to_string()),
    }
}

fn type_symbol_for_value_member_completion<'a>(
    resolved: &'a ResolveOutput,
    ty: &TypeExpr,
) -> Option<&'a TypeSymbol> {
    match ty {
        TypeExpr::Reference(reference) => resolved.type_symbol_by_reference_name(&reference.name),
        TypeExpr::Generic(generic) => resolved.type_symbol_by_reference_name(&generic.name),
        TypeExpr::Borrow(borrow) => {
            type_symbol_for_value_member_completion(resolved, &borrow.inner)
        }
        TypeExpr::View(view) => type_symbol_for_value_member_completion(resolved, &view.element),
        TypeExpr::Pointer(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn completion_context_at_offset(ast: &AstFile, offset: usize) -> Option<CompletionContext<'_>> {
    ast.items
        .iter()
        .find_map(|item| completion_context_in_item_at_offset(item, offset))
}

fn completion_context_in_item_at_offset(
    item: &Item,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    match item {
        Item::Function(function) => completion_context_in_block_at_offset(&function.body, offset),
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            ImplMember::Method(method) => method
                .body
                .as_ref()
                .and_then(|body| completion_context_in_block_at_offset(body, offset)),
            ImplMember::Drop(drop_) => completion_context_in_block_at_offset(&drop_.body, offset),
        }),
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Interface(_) => None,
    }
}

fn completion_context_in_block_at_offset(
    block: &Block,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    block
        .statements
        .iter()
        .find_map(|statement| completion_context_in_statement_at_offset(statement, offset))
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| completion_context_in_expression_at_offset(result, offset))
        })
}

fn completion_context_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    match statement {
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .and_then(|expression| completion_context_in_expression_at_offset(expression, offset)),
        Stmt::Binding(statement) => {
            completion_context_in_expression_at_offset(&statement.initializer, offset)
        }
        Stmt::Assignment(statement) => {
            completion_context_in_expression_at_offset(&statement.target, offset)
                .or_else(|| completion_context_in_expression_at_offset(&statement.value, offset))
        }
        Stmt::If(statement) => {
            completion_context_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.then_block, offset))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Stmt::IfIs(statement) => {
            enum_pattern_completion_context_in_if_is_at_offset(statement, offset)
                .or_else(|| {
                    completion_context_in_expression_at_offset(&statement.expression, offset)
                })
                .or_else(|| completion_context_in_block_at_offset(&statement.then_block, offset))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Stmt::Switch(statement) => completion_context_in_switch_at_offset(statement, offset)
            .or_else(|| completion_context_in_expression_at_offset(&statement.expression, offset)),
        Stmt::ForRange(statement) => {
            completion_context_in_expression_at_offset(&statement.start, offset)
                .or_else(|| completion_context_in_expression_at_offset(&statement.end, offset))
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::While(statement) => {
            completion_context_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Loop(statement) => completion_context_in_block_at_offset(&statement.body, offset),
        Stmt::Expression(statement) => {
            completion_context_in_expression_at_offset(&statement.expression, offset)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn completion_context_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    match expression {
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    completion_context_in_expression_at_offset(&part.expression, offset)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| completion_context_in_expression_at_offset(element, offset)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| completion_context_in_expression_at_offset(&field.value, offset))
            .or_else(|| struct_literal_field_completion_context_at_offset(expression, offset)),
        Expr::Propagate(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Force(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Catch(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.catch_block, offset))
        }
        Expr::Borrow(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Unary(expression) => {
            completion_context_in_expression_at_offset(&expression.operand, offset)
        }
        Expr::Binary(expression) => {
            completion_context_in_expression_at_offset(&expression.left, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.right, offset))
        }
        Expr::TypeConversion(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Call(expression) => {
            completion_context_in_expression_at_offset(&expression.callee, offset).or_else(|| {
                expression.arguments.iter().find_map(|argument| {
                    completion_context_in_expression_at_offset(argument, offset)
                })
            })
        }
        Expr::Member(expression) => {
            member_completion_context_in_member_expression_at_offset(expression, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.object, offset))
        }
        Expr::Index(expression) => {
            completion_context_in_expression_at_offset(&expression.object, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.index, offset))
        }
        Expr::Group(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Otherwise(expression) => {
            completion_context_in_expression_at_offset(&expression.value, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.fallback, offset))
        }
        Expr::If(expression) => {
            completion_context_in_expression_at_offset(&expression.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Expr::IfIs(expression) => {
            enum_pattern_completion_context_in_if_is_at_offset(expression, offset)
                .or_else(|| {
                    completion_context_in_expression_at_offset(&expression.expression, offset)
                })
                .or_else(|| completion_context_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Expr::Match(expression) => completion_context_in_switch_at_offset(expression, offset)
            .or_else(|| completion_context_in_expression_at_offset(&expression.expression, offset)),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn enum_pattern_completion_context_in_if_is_at_offset(
    statement: &IfIsStmt,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    offset_in_member_completion(
        statement.enum_name_span,
        statement.variant_name_span,
        offset,
    )
    .then_some(CompletionContext::EnumPatternMembers(
        statement.enum_name.as_str(),
    ))
}

fn completion_context_in_switch_at_offset(
    statement: &SwitchStmt,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    statement
        .arms
        .iter()
        .find_map(|arm| enum_pattern_completion_context_in_switch_arm_at_offset(arm, offset))
        .or_else(|| {
            statement
                .arms
                .iter()
                .find_map(|arm| completion_context_in_block_at_offset(&arm.body, offset))
        })
        .or_else(|| {
            statement
                .wildcard_arm
                .as_ref()
                .and_then(|arm| completion_context_in_block_at_offset(&arm.body, offset))
        })
}

fn enum_pattern_completion_context_in_switch_arm_at_offset(
    arm: &SwitchArm,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    offset_in_member_completion(arm.enum_name_span, arm.variant_name_span, offset).then_some(
        CompletionContext::EnumPatternMembers(arm.enum_name.as_str()),
    )
}

fn member_completion_context_in_member_expression_at_offset(
    expression: &MemberExpr,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    let Expr::Identifier(owner) = expression.object.without_groups() else {
        return None;
    };

    offset_in_member_completion(owner.span, expression.member_span, offset).then_some(
        CompletionContext::MemberAccess {
            owner_name: owner.name.as_str(),
            owner_span: owner.span,
        },
    )
}

fn struct_literal_field_completion_context_at_offset(
    literal: &StructLiteralExpr,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    if !span_contains(literal.fields_span, offset) {
        return None;
    }
    if literal
        .fields
        .iter()
        .any(|field| span_contains(field.value.span(), offset))
    {
        return None;
    }

    Some(CompletionContext::StructLiteralFields { literal, offset })
}

fn offset_in_member_completion(owner_span: ByteSpan, member_span: ByteSpan, offset: usize) -> bool {
    owner_span.source == member_span.source && owner_span.end < offset && offset <= member_span.end
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests;
