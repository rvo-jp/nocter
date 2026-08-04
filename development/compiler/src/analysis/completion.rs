//! Completion candidates derived from lexical keywords and resolver symbols.

mod context;
mod members;

use super::completion_recovery::completion_recovery_text;
use super::scoped_imports::visible_scoped_import_spans_at_offset;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::visible_locals::visible_local_bindings_at_offset;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, Block, Expr, IfIsStmt, ImplMember, Item, LiteralShape, MemberExpr, MethodReceiverMode,
    Stmt, StructLiteralExpr, SwitchArm, SwitchStmt, TypeExpr, substitute_type_expr_parameters,
};
use crate::lexer::KEYWORD_LEXEMES;
use crate::resolve::{
    AssociatedFunctionSignature, EnumVariantSignature, FunctionSignature, MethodSignature,
    ParameterSignature, ResolveOutput, StructFieldSignature, Symbol, SymbolKind, TypeSymbol,
    TypeSymbolKind,
};
use crate::source::ByteSpan;
use crate::source::SourceMap;
use crate::typecheck::{TypecheckFacts, collect_typecheck_facts};
use crate::typecheck::{
    default_method_completion_candidates, enum_variant_member_label, field_member_label,
    qualified_member_name, type_expr_is_aborting_allocator_capability,
    type_expr_presentation_label, type_symbol_presentation_label,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use context::completion_context_at_offset;
use members::{
    ValueMemberOwner, enum_variant_completion_items, member_completion_items,
    struct_literal_field_completion_items,
};

const CONTEXTUAL_COMPLETION_KEYWORDS: &[&str] = &["from"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionItemKind {
    Constructor,
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
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionItemInfo {
    pub(crate) label: String,
    pub(crate) kind: CompletionItemKind,
    pub(crate) detail: Option<String>,
    pub(crate) documentation: Option<String>,
    pub(crate) insert_text: Option<String>,
    pub(crate) sort_text: Option<String>,
    pub(crate) declaration_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionContext<'a> {
    LiteralShape(&'a TypeExpr),
    RegionAllocator,
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
    if let Some(items) = result_provenance_completion_items(&file.ast, offset) {
        return items;
    }
    if let Some(items) =
        contextual_completion_items(&file.ast, &file.resolved, &file.typecheck_facts, offset)
    {
        return items;
    }

    let mut items = local_completion_items(&file.ast, &file.typecheck_facts, offset);
    let local_names = items
        .iter()
        .map(|item| item.label.clone())
        .collect::<HashSet<_>>();
    items.extend(completion_items_for_resolved_symbols_excluding(
        &file.resolved,
        visible_scoped_import_spans_at_offset(&file.ast, offset),
        &local_names,
    ));
    items
}

pub(crate) fn literal_shape_completion_items_for_file_analysis_at_offset(
    file: &FileAnalysis,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let CompletionContext::LiteralShape(target) = completion_context_at_offset(&file.ast, offset)?
    else {
        return None;
    };
    let items = literal_shape_completion_items(&file.resolved, target);
    (!items.is_empty()).then_some(items)
}

pub(crate) fn completion_items_for_compile_unit_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    let mut items = super::import_completion::import_symbol_items_at_offset(analysis, file, offset)
        .unwrap_or_else(|| completion_items_for_file_analysis_at_offset(file, offset));
    let compatible_locals = super::expected_completion::compatible_local_spans_at_offset(
        sources, analysis, file, offset,
    );
    let prefix = sources
        .get(file.ast.span.source)
        .map(|source| identifier_prefix_at_offset(source.text(), offset))
        .unwrap_or_default();
    for item in &mut items {
        let expected_rank = if item
            .declaration_span
            .is_some_and(|span| compatible_locals.contains(&span))
        {
            0
        } else {
            1
        };
        let prefix_rank = usize::from(!prefix.is_empty() && !item.label.starts_with(prefix));
        let locality_rank = usize::from(item.kind != CompletionItemKind::Variable);
        item.sort_text = Some(format!(
            "{prefix_rank}{locality_rank}{expected_rank}-{}",
            item.label
        ));
        let Some(target) = item.declaration_span else {
            continue;
        };
        let Some(target_file) = analysis.file_by_source(target.source) else {
            continue;
        };
        item.documentation =
            super::hover::hover_for_file_analysis(sources, analysis, target_file, target.start)
                .and_then(|hover| hover.documentation);
    }
    items
}

fn identifier_prefix_at_offset(text: &str, offset: usize) -> &str {
    let Some(prefix) = text.get(..offset) else {
        return "";
    };
    let start = prefix
        .char_indices()
        .rev()
        .find(|(_, char)| !(*char == '_' || char.is_alphanumeric()))
        .map_or(0, |(index, char)| index + char.len_utf8());
    &prefix[start..]
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

    if let Some(items) = result_provenance_completion_items(&parsed.ast, offset) {
        return Some(items);
    }

    if let Some(items) = contextual_completion_items(&parsed.ast, &resolved, &facts, offset) {
        return Some(items);
    }

    let mut items = local_completion_items(&parsed.ast, &facts, offset);
    let local_names = items
        .iter()
        .map(|item| item.label.clone())
        .collect::<HashSet<_>>();
    items.extend(completion_items_for_resolved_symbols_excluding(
        &resolved,
        visible_scoped_import_spans_at_offset(&parsed.ast, offset),
        &local_names,
    ));
    Some(items)
}

fn result_provenance_completion_items(
    ast: &AstFile,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                if clause_contains_offset(function.result_provenance.as_ref(), offset) {
                    return Some(provenance_origin_items(
                        None,
                        &function.parameters.parameters,
                    ));
                }
            }
            Item::Primitive(primitive) => {
                if clause_contains_offset(primitive.result_provenance.as_ref(), offset) {
                    return Some(provenance_origin_items(
                        None,
                        &primitive.parameters.parameters,
                    ));
                }
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if clause_contains_offset(method.result_provenance.as_ref(), offset) {
                        return Some(provenance_origin_items(
                            Some(method.receiver.mode),
                            &method.parameters.parameters,
                        ));
                    }
                }
            }
            Item::Impl(impl_) => {
                for member in &impl_.members {
                    let ImplMember::Method(method) = member else {
                        continue;
                    };
                    if clause_contains_offset(method.result_provenance.as_ref(), offset) {
                        return Some(provenance_origin_items(
                            Some(method.receiver.mode),
                            &method.parameters.parameters,
                        ));
                    }
                }
            }
            Item::Literal(literal) => {
                if clause_contains_offset(literal.result_provenance.as_ref(), offset) {
                    return Some(provenance_origin_items(
                        None,
                        &literal.parameters.parameters,
                    ));
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
    None
}

fn clause_contains_offset(
    clause: Option<&crate::ast::ResultProvenanceClause>,
    offset: usize,
) -> bool {
    clause.is_some_and(|clause| clause.span.start <= offset && offset <= clause.span.end)
}

fn provenance_origin_items(
    receiver: Option<MethodReceiverMode>,
    parameters: &[crate::ast::Parameter],
) -> Vec<CompletionItemInfo> {
    let mut labels = Vec::new();
    if receiver.is_some_and(|mode| mode != MethodReceiverMode::Owned) {
        labels.push(("self".to_string(), CompletionItemKind::Variable));
    }
    labels.extend(
        parameters
            .iter()
            .filter(|parameter| matches!(parameter.ty, TypeExpr::Borrow(_) | TypeExpr::View(_)))
            .map(|parameter| (parameter.name.clone(), CompletionItemKind::Variable)),
    );
    labels.extend([
        ("static".to_string(), CompletionItemKind::Keyword),
        ("current".to_string(), CompletionItemKind::Keyword),
    ]);
    labels
        .into_iter()
        .map(|(label, kind)| CompletionItemInfo {
            insert_text: Some(label.clone()),
            label,
            kind,
            detail: Some("result provenance origin".to_string()),
            documentation: None,
            sort_text: None,
            declaration_span: None,
        })
        .collect()
}

pub(crate) fn keyword_completion_items() -> Vec<CompletionItemInfo> {
    KEYWORD_LEXEMES
        .iter()
        .chain(CONTEXTUAL_COMPLETION_KEYWORDS.iter())
        .map(|keyword| CompletionItemInfo {
            label: (*keyword).to_string(),
            kind: CompletionItemKind::Keyword,
            detail: Some("keyword".to_string()),
            documentation: None,
            insert_text: Some((*keyword).to_string()),
            sort_text: None,
            declaration_span: None,
        })
        .collect()
}

#[cfg(test)]
fn completion_items_for_resolved_symbols(
    resolved: &ResolveOutput,
    visible_hidden_symbol_spans: HashSet<ByteSpan>,
) -> Vec<CompletionItemInfo> {
    completion_items_for_resolved_symbols_excluding(
        resolved,
        visible_hidden_symbol_spans,
        &HashSet::new(),
    )
}

fn completion_items_for_resolved_symbols_excluding(
    resolved: &ResolveOutput,
    visible_hidden_symbol_spans: HashSet<ByteSpan>,
    excluded_names: &HashSet<String>,
) -> Vec<CompletionItemInfo> {
    let mut items = keyword_completion_items();
    let mut seen = KEYWORD_LEXEMES
        .iter()
        .chain(CONTEXTUAL_COMPLETION_KEYWORDS.iter())
        .map(|keyword| (*keyword).to_string())
        .collect::<HashSet<_>>();
    seen.extend(excluded_names.iter().cloned());

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
            detail: Some(symbol_detail(symbol, resolved)),
            documentation: None,
            insert_text: Some(symbol_insert_text(symbol)),
            sort_text: None,
            declaration_span: Some(symbol.declaration_span),
        });
    }

    items
}

fn local_completion_items(
    ast: &AstFile,
    facts: &TypecheckFacts,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    visible_local_bindings_at_offset(ast, offset)
        .into_iter()
        .map(|binding| {
            let name = binding.name;
            let detail = facts
                .binding_type_label(binding.name_span)
                .map(|ty| format!("{} {name}: {ty}", binding.kind))
                .unwrap_or_else(|| format!("{} {name}", binding.kind));
            CompletionItemInfo {
                label: name.clone(),
                kind: CompletionItemKind::Variable,
                detail: Some(detail),
                documentation: None,
                insert_text: Some(name),
                sort_text: None,
                declaration_span: Some(binding.name_span),
            }
        })
        .collect()
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

pub(super) fn completion_item_for_symbol(
    symbol: &Symbol,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    CompletionItemInfo {
        label: symbol.name.clone(),
        kind: completion_kind_for_symbol(symbol),
        detail: Some(symbol_detail(symbol, resolved)),
        documentation: None,
        insert_text: Some(symbol_insert_text(symbol)),
        sort_text: None,
        declaration_span: Some(symbol.declaration_span),
    }
}

fn symbol_detail(symbol: &Symbol, resolved: &ResolveOutput) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => {
            callable_detail("func", &symbol.name, signature, resolved)
        }
        SymbolKind::Primitive(signature) => {
            callable_detail("primitive", &symbol.name, signature, resolved)
        }
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => format!("type {}{}", symbol.name, generic_suffix(type_symbol)),
            TypeSymbolKind::Struct => {
                format!("struct {}{}", symbol.name, generic_suffix(type_symbol))
            }
            TypeSymbolKind::Enum => format!("enum {}{}", symbol.name, generic_suffix(type_symbol)),
            TypeSymbolKind::Interface => {
                format!("interface {}{}", symbol.name, generic_suffix(type_symbol))
            }
        },
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

fn symbol_insert_text(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => format!("{}()", symbol.name),
        SymbolKind::Type(_) | SymbolKind::Imported(_) => symbol.name.clone(),
    }
}

fn generic_suffix(symbol: &TypeSymbol) -> String {
    if symbol.generic_parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", symbol.generic_parameters.join(", "))
    }
}

fn callable_detail(
    kind: &str,
    name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> String {
    let generics = if signature.generic_parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", signature.generic_parameters.join(", "))
    };
    format!(
        "{kind} {name}{generics}({}): {}",
        signature
            .parameters
            .iter()
            .map(|parameter| parameter_detail(parameter, resolved))
            .collect::<Vec<_>>()
            .join(", "),
        type_expr_presentation_label(&signature.return_type, resolved)
    )
}

fn parameter_detail(parameter: &ParameterSignature, resolved: &ResolveOutput) -> String {
    format!(
        "{}: {}",
        parameter.name,
        type_expr_presentation_label(&parameter.ty, resolved)
    )
}

fn contextual_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    match completion_context_at_offset(ast, offset)? {
        CompletionContext::LiteralShape(target) => {
            let items = literal_shape_completion_items(resolved, target);
            (!items.is_empty()).then_some(items)
        }
        CompletionContext::RegionAllocator => Some(region_allocator_completion_items(
            ast, resolved, facts, offset,
        )),
        CompletionContext::EnumPatternMembers(enum_name) => Some(
            resolved
                .type_symbol_by_name(enum_name)
                .map(|symbol| enum_variant_completion_items(symbol, resolved))
                .unwrap_or_default(),
        ),
        CompletionContext::MemberAccess {
            owner_name,
            owner_span,
        } => Some(member_completion_items(
            ast, resolved, facts, owner_name, owner_span, offset,
        )),
        CompletionContext::StructLiteralFields { literal, offset } => Some(
            struct_literal_field_completion_items(resolved, literal, offset),
        ),
    }
}

fn literal_shape_completion_items(
    resolved: &ResolveOutput,
    target: &TypeExpr,
) -> Vec<CompletionItemInfo> {
    let Some(owner) = literal_owner(resolved, target) else {
        return Vec::new();
    };
    let target_label = match target {
        TypeExpr::Reference(reference) if !owner.symbol.generic_parameters.is_empty() => format!(
            "{}<{}>",
            reference.name,
            owner.symbol.generic_parameters.join(", ")
        ),
        _ => type_expr_presentation_label(target, resolved),
    };

    owner
        .symbol
        .literals
        .iter()
        .filter(|literal| literal.is_accessible)
        .map(|literal| {
            let (label, parameters) = match literal.shape {
                LiteralShape::Sequence => (
                    "[]",
                    literal
                        .capture
                        .as_ref()
                        .map(|capture| {
                            let ty = substitute_type_expr_parameters(
                                &capture.element_type,
                                &owner.substitutions,
                            );
                            format!(
                                "...{}: {}",
                                capture.name,
                                type_expr_presentation_label(&ty, resolved)
                            )
                        })
                        .into_iter()
                        .collect::<Vec<_>>(),
                ),
                LiteralShape::String => (
                    "\"\"",
                    literal
                        .parameters
                        .iter()
                        .map(|parameter| {
                            let ty = substitute_type_expr_parameters(
                                &parameter.ty,
                                &owner.substitutions,
                            );
                            format!(
                                "{}: {}",
                                parameter.name,
                                type_expr_presentation_label(&ty, resolved)
                            )
                        })
                        .collect(),
                ),
            };
            CompletionItemInfo {
                label: label.to_string(),
                kind: CompletionItemKind::Constructor,
                detail: Some(format!(
                    "literal {target_label} {label}({}): {target_label}",
                    parameters.join(", ")
                )),
                documentation: None,
                insert_text: Some(label.to_string()),
                sort_text: None,
                declaration_span: Some(literal.shape_span),
            }
        })
        .collect()
}

fn literal_owner<'a>(
    resolved: &'a ResolveOutput,
    target: &TypeExpr,
) -> Option<ValueMemberOwner<'a>> {
    match target {
        TypeExpr::Closure(_) => None,
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            Some(ValueMemberOwner {
                symbol,
                substitutions: HashMap::from([("Self".to_string(), target.clone())]),
            })
        }
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            let mut substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect::<HashMap<_, _>>();
            substitutions.insert("Self".to_string(), target.clone());
            Some(ValueMemberOwner {
                symbol,
                substitutions,
            })
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn region_allocator_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    local_completion_items(ast, facts, offset)
        .into_iter()
        .filter(|item| {
            item.declaration_span
                .and_then(|span| facts.binding_type_expr(span))
                .is_some_and(|ty| type_expr_is_aborting_allocator_capability(ty, resolved))
        })
        .collect()
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests;
