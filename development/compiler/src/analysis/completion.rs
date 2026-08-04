//! Completion candidates derived from lexical keywords and resolver symbols.

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

fn member_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    owner_name: &str,
    owner_span: ByteSpan,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    if let Some(symbol) = resolved.type_symbol_by_name(owner_name) {
        return type_member_completion_items(symbol, resolved);
    }

    let Some(owner_ty) = facts.expression_type_expr(owner_span) else {
        return Vec::new();
    };
    let can_readwrite = owner_type_is_readwrite(owner_ty)
        || (!matches!(owner_ty, TypeExpr::Borrow(_))
            && !facts.binding_is_readonly(owner_span).unwrap_or(true));
    let can_move = !matches!(owner_ty, TypeExpr::Borrow(_));
    if let Some(owner) = value_member_owner(resolved, owner_ty) {
        return value_member_completion_items(
            &owner,
            resolved,
            can_readwrite,
            can_move,
            owner_span.source,
        );
    }
    let owners = generic_bound_member_owners(ast, resolved, owner_ty, offset);
    unambiguous_capability_member_items(
        owners,
        resolved,
        can_readwrite,
        can_move,
        owner_span.source,
    )
}

fn type_member_completion_items(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Vec<CompletionItemInfo> {
    let owner = type_symbol_presentation_label(symbol, resolved);
    let mut items = Vec::new();
    if symbol.kind == TypeSymbolKind::Enum {
        items.extend(enum_variant_completion_items(symbol, resolved));
    }
    items.extend(
        symbol
            .associated_functions
            .iter()
            .filter(|function| function.is_accessible)
            .map(|function| associated_function_completion_item(function, &owner, resolved)),
    );
    items
}

fn value_member_completion_items(
    owner: &ValueMemberOwner<'_>,
    resolved: &ResolveOutput,
    can_readwrite: bool,
    can_move: bool,
    use_source: crate::source::SourceId,
) -> Vec<CompletionItemInfo> {
    let mut items = Vec::new();
    items.extend(
        owner
            .symbol
            .fields
            .iter()
            .filter(|field| field.is_accessible)
            .map(|field| {
                struct_field_completion_item(field, resolved, false, &owner.substitutions)
            }),
    );
    items.extend(
        owner
            .symbol
            .methods
            .iter()
            .filter(|method| {
                method.is_accessible
                    && method_receiver_is_available(method, can_readwrite, can_move)
            })
            .map(|method| method_completion_item(method, resolved, &owner.substitutions)),
    );
    let inherent_names = owner
        .symbol
        .methods
        .iter()
        .map(|method| method.name.as_str())
        .collect::<HashSet<_>>();
    let self_ty = owner
        .substitutions
        .get("Self")
        .expect("value member owners always define Self");
    let mut defaults_by_name: HashMap<&str, Vec<CompletionItemInfo>> = HashMap::new();
    for candidate in default_method_completion_candidates(self_ty, use_source, resolved) {
        if inherent_names.contains(candidate.method.name.as_str())
            || !method_receiver_is_available(candidate.method, can_readwrite, can_move)
        {
            continue;
        }
        defaults_by_name
            .entry(candidate.method.name.as_str())
            .or_default()
            .push(method_completion_item(
                candidate.method,
                resolved,
                &candidate.substitutions,
            ));
    }
    items.extend(defaults_by_name.into_values().filter_map(|candidates| {
        let identities = candidates
            .iter()
            .filter_map(|item| item.declaration_span)
            .collect::<HashSet<_>>();
        (identities.len() == 1).then(|| candidates.into_iter().next().unwrap())
    }));
    items
}

fn struct_literal_field_completion_items(
    resolved: &ResolveOutput,
    literal: &StructLiteralExpr,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    let Some(owner) = value_member_owner(resolved, &literal.ty) else {
        return Vec::new();
    };
    let used_fields = literal
        .fields
        .iter()
        .filter(|field| !span_contains(field.name_span, offset))
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();

    owner
        .symbol
        .fields
        .iter()
        .filter(|field| field.is_accessible && !used_fields.contains(field.name.as_str()))
        .map(|field| struct_field_completion_item(field, resolved, true, &owner.substitutions))
        .collect()
}

fn enum_variant_completion_items(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Vec<CompletionItemInfo> {
    let owner = type_symbol_presentation_label(symbol, resolved);
    symbol
        .variants
        .iter()
        .map(|variant| enum_variant_completion_item(variant, &owner, resolved))
        .collect()
}

fn enum_variant_completion_item(
    variant: &EnumVariantSignature,
    owner: &str,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    let payload = variant
        .payload
        .iter()
        .map(|parameter| parameter_detail(parameter, resolved))
        .collect::<Vec<_>>();
    CompletionItemInfo {
        label: variant.name.clone(),
        kind: CompletionItemKind::EnumMember,
        detail: Some(enum_variant_member_label(owner, &variant.name, &payload)),
        documentation: None,
        insert_text: Some(if payload.is_empty() {
            variant.name.clone()
        } else {
            format!("{}(_)", variant.name)
        }),
        sort_text: None,
        declaration_span: Some(variant.name_span),
    }
}

fn associated_function_completion_item(
    function: &AssociatedFunctionSignature,
    owner: &str,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    CompletionItemInfo {
        label: function.name.clone(),
        kind: CompletionItemKind::Function,
        detail: Some(callable_detail(
            "func",
            &qualified_member_name(owner, &function.name),
            &function.signature,
            resolved,
        )),
        documentation: None,
        insert_text: Some(format!("{}()", function.name)),
        sort_text: None,
        declaration_span: Some(function.name_span),
    }
}

fn struct_field_completion_item(
    field: &StructFieldSignature,
    resolved: &ResolveOutput,
    literal: bool,
    substitutions: &HashMap<String, TypeExpr>,
) -> CompletionItemInfo {
    let ty = substitute_type_expr_parameters(&field.ty, substitutions);
    let owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    CompletionItemInfo {
        label: field.name.clone(),
        kind: CompletionItemKind::Field,
        detail: Some(field_member_label(
            &owner,
            &field.name,
            &type_expr_presentation_label(&ty, resolved),
        )),
        documentation: None,
        insert_text: Some(if literal {
            format!("{}: ", field.name)
        } else {
            field.name.clone()
        }),
        sort_text: None,
        declaration_span: Some(field.name_span),
    }
}

fn method_completion_item(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, TypeExpr>,
) -> CompletionItemInfo {
    let mut substitutions = substitutions.clone();
    if let Some(impl_target) = &method.impl_target_ty {
        let impl_target = substitute_type_expr_parameters(impl_target, &substitutions);
        substitutions.insert("Self".to_string(), impl_target);
    }
    let receiver_owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    let receiver = format!("{}{receiver_owner}", method.receiver.mode.source_prefix());
    let return_type =
        substitute_type_expr_parameters(&method.signature.return_type, &substitutions);
    let mut detail = format!(
        "method {}.{}({}): {}",
        receiver,
        method.name,
        method
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                let ty = substitute_type_expr_parameters(&parameter.ty, &substitutions);
                format!(
                    "{}: {}",
                    parameter.name,
                    type_expr_presentation_label(&ty, resolved)
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
        type_expr_presentation_label(&return_type, resolved)
    );
    if let Some(clause) = &method.signature.result_provenance {
        detail.push_str(" from ");
        detail.push_str(
            &clause
                .origins
                .iter()
                .map(|origin| origin.kind.source_label())
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    CompletionItemInfo {
        label: method.name.clone(),
        kind: CompletionItemKind::Method,
        detail: Some(detail),
        documentation: None,
        insert_text: Some(format!("{}()", method.name)),
        sort_text: None,
        declaration_span: Some(method.name_span),
    }
}

fn generic_bound_member_owners<'a>(
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
    ty: &TypeExpr,
    offset: usize,
) -> Vec<ValueMemberOwner<'a>> {
    let Some(parameter_name) = borrowed_reference_name(ty) else {
        return Vec::new();
    };
    generic_bounds_at_offset(ast, parameter_name, offset)
        .into_iter()
        .filter_map(|bound| {
            let mut owner = value_member_owner(resolved, bound)?;
            owner.substitutions.insert(
                "Self".to_string(),
                TypeExpr::Reference(crate::ast::TypeReference {
                    span: ty.span(),
                    name: parameter_name.to_string(),
                }),
            );
            Some(owner)
        })
        .collect()
}

fn borrowed_reference_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Borrow(borrow) => borrowed_reference_name(&borrow.inner),
        _ => None,
    }
}

fn generic_bounds_at_offset<'a>(
    ast: &'a AstFile,
    parameter_name: &str,
    offset: usize,
) -> Vec<&'a TypeExpr> {
    ast.items
        .iter()
        .find_map(|item| {
            let generics = match item {
                Item::Function(function) if span_contains(function.body.span, offset) => {
                    &function.generics
                }
                Item::Impl(impl_) if span_contains(impl_.span, offset) => &impl_.generics,
                _ => return None,
            };
            generics
                .parameters
                .iter()
                .find(|parameter| parameter.name == parameter_name)
                .map(|parameter| parameter.bounds.iter().collect())
        })
        .unwrap_or_default()
}

fn unambiguous_capability_member_items(
    owners: Vec<ValueMemberOwner<'_>>,
    resolved: &ResolveOutput,
    can_readwrite: bool,
    can_move: bool,
    use_source: crate::source::SourceId,
) -> Vec<CompletionItemInfo> {
    let mut by_label: HashMap<String, Vec<CompletionItemInfo>> = HashMap::new();
    for owner in owners {
        for item in
            value_member_completion_items(&owner, resolved, can_readwrite, can_move, use_source)
        {
            by_label.entry(item.label.clone()).or_default().push(item);
        }
    }
    let mut items = by_label
        .into_values()
        .filter_map(|candidates| {
            let identities = candidates
                .iter()
                .filter_map(|item| item.declaration_span)
                .collect::<HashSet<_>>();
            (identities.len() == 1).then(|| candidates.into_iter().next().unwrap())
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}

struct ValueMemberOwner<'a> {
    symbol: &'a TypeSymbol,
    substitutions: HashMap<String, TypeExpr>,
}

fn value_member_owner<'a>(
    resolved: &'a ResolveOutput,
    ty: &TypeExpr,
) -> Option<ValueMemberOwner<'a>> {
    match ty {
        TypeExpr::Closure(_) => None,
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            Some(ValueMemberOwner {
                symbol,
                substitutions: HashMap::from([("Self".to_string(), ty.clone())]),
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
            substitutions.insert("Self".to_string(), ty.clone());
            Some(ValueMemberOwner {
                symbol,
                substitutions,
            })
        }
        TypeExpr::Borrow(borrow) => value_member_owner(resolved, &borrow.inner),
        TypeExpr::View(view) => value_member_owner(resolved, &view.element),
        TypeExpr::Pointer(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn owner_type_is_readwrite(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Borrow(borrow) if borrow.is_readwrite)
}

fn method_receiver_is_available(
    method: &MethodSignature,
    can_readwrite: bool,
    can_move: bool,
) -> bool {
    match method.receiver.mode {
        MethodReceiverMode::ReadwriteBorrow => can_readwrite,
        MethodReceiverMode::ReadonlyBorrow => true,
        MethodReceiverMode::Owned => can_move,
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
        Item::Literal(literal) => completion_context_in_block_at_offset(&literal.body, offset),
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            ImplMember::Method(method) => method
                .body
                .as_ref()
                .and_then(|body| completion_context_in_block_at_offset(body, offset)),
            ImplMember::Drop(drop_) => completion_context_in_block_at_offset(&drop_.body, offset),
        }),
        Item::Interface(interface) => interface.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| completion_context_in_block_at_offset(body, offset))
        }),
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_) => None,
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
        Stmt::CollectionFor(statement) => {
            completion_context_in_expression_at_offset(&statement.source, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::LiteralPackFor(statement) => {
            completion_context_in_block_at_offset(&statement.body, offset)
        }
        Stmt::While(statement) => {
            completion_context_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Loop(statement) => completion_context_in_block_at_offset(&statement.body, offset),
        Stmt::Region(statement) => {
            completion_context_in_expression_at_offset(&statement.allocator, offset)
                .or_else(|| {
                    cursor_touches_span(statement.allocator.span(), offset)
                        .then_some(CompletionContext::RegionAllocator)
                })
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
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

fn cursor_touches_span(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn completion_context_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
) -> Option<CompletionContext<'_>> {
    match expression {
        Expr::Closure(expression) => {
            completion_context_in_block_at_offset(&expression.body, offset)
        }
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
        Expr::TypedSequenceLiteral(expression) => (expression.target.span().end <= offset)
            .then_some(CompletionContext::LiteralShape(&expression.target))
            .filter(|_| offset <= expression.elements_span.start)
            .or_else(|| {
                expression
                    .elements
                    .iter()
                    .find_map(|element| completion_context_in_expression_at_offset(element, offset))
            })
            .or_else(|| {
                expression.using.as_ref().and_then(|using| {
                    completion_context_in_expression_at_offset(&using.allocator, offset)
                })
            }),
        Expr::TypedStringLiteral(expression) => (expression.target.span().end <= offset
            && offset <= expression.text.span.start)
            .then_some(CompletionContext::LiteralShape(&expression.target))
            .or_else(|| {
                expression.using.as_ref().and_then(|using| {
                    completion_context_in_expression_at_offset(&using.allocator, offset)
                })
            }),
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
