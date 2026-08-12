//! Completion candidates derived from lexical keywords and resolver symbols.

mod context;
mod members;

use super::completion_recovery::completion_recovery_text;
use super::scoped_imports::visible_scoped_import_spans_at_offset;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::visible_locals::visible_local_bindings_at_offset;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, Block, Expr, IfIsStmt, Item, LiteralShape, MemberExpr, MethodReceiverMode, Stmt,
    StructLiteralExpr, SwitchArm, SwitchStmt, TypeExpr, substitute_type_expr_parameters,
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
    enum_variant_member_label, field_member_label, interface_method_completion_candidates,
    type_expr_is_aborting_allocator_capability, type_expr_presentation_label,
    type_symbol_presentation_label,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use context::completion_context_at_offset;
use members::{
    ValueMemberOwner, enum_variant_completion_items, member_completion_items,
    struct_literal_field_completion_items,
};

const CONTEXTUAL_COMPLETION_KEYWORDS: &[&str] = &["from", "copy"];

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
    if let Some(items) =
        associated_type_completion_items(&file.ast, &file.resolved, &file.typecheck_facts, offset)
    {
        return items;
    }
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
    if !copy_requirement_completion_is_allowed(&file.ast, offset) {
        items.retain(|item| item.label != "copy");
    }
    apply_operator_completion(&file.ast, offset, &mut items);
    apply_operator_requirement_completion(&file.ast, offset, &mut items);
    items
}

fn copy_requirement_completion_is_allowed(ast: &AstFile, offset: usize) -> bool {
    fn clause(clause: Option<&crate::ast::WhereClause>, offset: usize) -> bool {
        clause.is_some_and(|clause| {
            clause.span.start <= offset
                && offset <= clause.span.end
                && !clause.predicates.iter().any(|predicate| {
                    let span = match predicate {
                        crate::ast::WherePredicate::Copy(requirement) => requirement.span,
                        crate::ast::WherePredicate::Generic(requirement) => requirement.span,
                        crate::ast::WherePredicate::Refinement(refinement) => refinement.span,
                        crate::ast::WherePredicate::Equality(equality) => equality.span,
                        crate::ast::WherePredicate::Operator(requirement) => requirement.span,
                        crate::ast::WherePredicate::Coercion(requirement) => requirement.span,
                    };
                    span.start < offset && offset <= span.end
                })
        })
    }
    fn method(method: &crate::ast::MethodDecl, offset: usize) -> bool {
        clause(method.requirements.as_ref(), offset)
    }
    ast.items.iter().any(|item| match item {
        Item::Function(function) => clause(function.requirements.as_ref(), offset),
        Item::Primitive(primitive) => clause(primitive.requirements.as_ref(), offset),
        Item::TypeAlias(alias) => clause(alias.requirements.as_ref(), offset),
        Item::Struct(struct_) => clause(struct_.requirements.as_ref(), offset),
        Item::Enum(enum_) => clause(enum_.requirements.as_ref(), offset),
        Item::Interface(interface) => {
            clause(interface.requirements.as_ref(), offset)
                || interface
                    .methods
                    .iter()
                    .any(|member| method(member, offset))
        }
        Item::Instance(_) | Item::Conformance(_) => {
            let owner = item.method_owner().expect("matched method owner");
            clause(owner.requirements(), offset)
                || owner.methods().any(|member| method(member, offset))
        }
        Item::Construct(construct) => {
            construct
                .functions()
                .any(|(_, function)| clause(function.requirements.as_ref(), offset))
                || construct
                    .literals()
                    .any(|(_, literal)| clause(literal.requirements.as_ref(), offset))
        }
        Item::Destruct(_) => false,
        Item::Import(_) | Item::FromImport(_) | Item::Test(_) => false,
    })
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
        let is_default_construction = item
            .sort_text
            .as_deref()
            .is_some_and(|sort| sort.starts_with("0-"));
        let expected_rank = if item
            .declaration_span
            .is_some_and(|span| compatible_locals.contains(&span))
        {
            0
        } else {
            1
        };
        let prefix_rank = usize::from(!prefix.is_empty() && !item.label.starts_with(prefix));
        let locality_rank =
            usize::from(item.kind != CompletionItemKind::Variable && !is_default_construction);
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

    if let Some(items) = associated_type_completion_items(&parsed.ast, &resolved, &facts, offset) {
        return Some(items);
    }

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
    if !copy_requirement_completion_is_allowed(&parsed.ast, offset) {
        items.retain(|item| item.label != "copy");
    }
    apply_operator_completion(&parsed.ast, offset, &mut items);
    apply_operator_requirement_completion(&parsed.ast, offset, &mut items);
    Some(items)
}

fn apply_operator_requirement_completion(
    ast: &AstFile,
    offset: usize,
    items: &mut Vec<CompletionItemInfo>,
) {
    fn clause_contains(clause: Option<&crate::ast::WhereClause>, offset: usize) -> bool {
        clause.is_some_and(|clause| {
            clause.operator_requirements().any(|requirement| {
                requirement.open_paren_span.end <= offset && offset <= requirement.span.end
            })
        })
    }
    fn method_contains(method: &crate::ast::MethodDecl, offset: usize) -> bool {
        clause_contains(method.requirements.as_ref(), offset)
    }
    let active = ast.items.iter().any(|item| match item {
        Item::Function(function) => clause_contains(function.requirements.as_ref(), offset),
        Item::Primitive(primitive) => clause_contains(primitive.requirements.as_ref(), offset),
        Item::TypeAlias(alias) => clause_contains(alias.requirements.as_ref(), offset),
        Item::Struct(struct_) => clause_contains(struct_.requirements.as_ref(), offset),
        Item::Enum(enum_) => clause_contains(enum_.requirements.as_ref(), offset),
        Item::Interface(interface) => {
            clause_contains(interface.requirements.as_ref(), offset)
                || interface
                    .methods
                    .iter()
                    .any(|method| method_contains(method, offset))
        }
        Item::Instance(_) | Item::Conformance(_) => {
            let owner = item.method_owner().expect("matched method owner");
            clause_contains(owner.requirements(), offset)
                || owner
                    .methods()
                    .any(|method| method_contains(method, offset))
        }
        Item::Construct(construct) => {
            construct
                .functions()
                .any(|(_, function)| clause_contains(function.requirements.as_ref(), offset))
                || construct
                    .literals()
                    .any(|(_, literal)| clause_contains(literal.requirements.as_ref(), offset))
        }
        Item::Destruct(_) | Item::Import(_) | Item::FromImport(_) | Item::Test(_) => false,
    });
    if !active {
        return;
    }
    for parameter in ast.items.iter().flat_map(|item| match item {
        Item::Function(function) if clause_contains(function.requirements.as_ref(), offset) => {
            function.generics.parameters.as_slice()
        }
        Item::Primitive(primitive) if clause_contains(primitive.requirements.as_ref(), offset) => {
            primitive.generics.parameters.as_slice()
        }
        Item::TypeAlias(alias) if clause_contains(alias.requirements.as_ref(), offset) => {
            alias.generics.parameters.as_slice()
        }
        Item::Struct(struct_) if clause_contains(struct_.requirements.as_ref(), offset) => {
            struct_.generics.parameters.as_slice()
        }
        Item::Enum(enum_) if clause_contains(enum_.requirements.as_ref(), offset) => {
            enum_.generics.parameters.as_slice()
        }
        Item::Interface(interface) if clause_contains(interface.requirements.as_ref(), offset) => {
            interface.generics.parameters.as_slice()
        }
        _ => &[],
    }) {
        if items.iter().any(|item| item.label == parameter.name) {
            continue;
        }
        items.push(CompletionItemInfo {
            label: parameter.name.clone(),
            kind: CompletionItemKind::Class,
            detail: Some("generic type parameter".to_string()),
            documentation: None,
            insert_text: None,
            sort_text: None,
            declaration_span: Some(parameter.name_span),
        });
    }
    for (label, detail, insert_text) in [
        ("==", "equality operator requirement", "&T == &T): bool"),
        ("<", "strict-order operator requirement", "&T < &T): bool"),
        ("[]", "index operator requirement", "&C[K]): &V"),
        ("...", "expansion operator requirement", "...&C): I"),
    ] {
        if items.iter().any(|item| item.label == label) {
            continue;
        }
        items.push(CompletionItemInfo {
            label: label.to_string(),
            kind: CompletionItemKind::Keyword,
            detail: Some(detail.to_string()),
            documentation: None,
            insert_text: Some(insert_text.to_string()),
            sort_text: None,
            declaration_span: None,
        });
    }
}

fn apply_operator_completion(ast: &AstFile, offset: usize, items: &mut Vec<CompletionItemInfo>) {
    items.retain(|item| item.label != "operator");
    let Some(instance) = ast.items.iter().find_map(|item| {
        let Item::Instance(instance) = item else {
            return None;
        };
        (instance.target_ty.span().end < offset
            && offset < instance.span.end
            && instance
                .callable_methods()
                .all(|method| !(method.span.start <= offset && offset <= method.span.end)))
        .then_some(instance)
    }) else {
        return;
    };
    let owner = crate::ast::canonical_type_expr(&instance.target_ty);
    let has_equality = instance.operators.iter().any(|operator| {
        matches!(
            operator,
            crate::ast::OperatorDecl::Comparison(operator)
                if operator.kind == crate::ast::ComparisonOperatorKind::Equality
        )
    });
    let has_ordering = instance.ordering_operators().next().is_some();
    let has_readonly_index = instance.index_operators().any(|operator| {
        operator.callable_method().receiver.mode == MethodReceiverMode::ReadonlyBorrow
    });
    let has_readwrite_index = instance.index_operators().any(|operator| {
        operator.callable_method().receiver.mode == MethodReceiverMode::ReadwriteBorrow
    });
    let has_expansion = |mode| {
        instance
            .expansion_operators()
            .any(|operator| operator.callable_method().receiver.mode == mode)
    };
    let candidates = [
        (
            !has_equality,
            format!("operator (&{owner} == other: &{owner}): bool"),
            "Declares readonly homogeneous equality for this instance. `!=` is derived automatically.",
            "operator (&self == other: &Self): bool {\n    return false\n}",
        ),
        (
            !has_ordering,
            format!("operator (&{owner} < other: &{owner}): bool"),
            "Declares readonly homogeneous strict ordering. `>`, `<=`, and `>=` are derived automatically.",
            "operator (&self < other: &Self): bool {\n    return false\n}",
        ),
        (
            !has_readonly_index,
            format!("operator (&{owner}[index: usize]): &Element"),
            "Declares readonly indexing for this instance.",
            "operator (&self[index: usize]): &Element {\n    return &self.values[index]\n}",
        ),
        (
            !has_readwrite_index,
            format!("operator (&+{owner}[index: usize]): &+Element"),
            "Declares readwrite indexing for this instance.",
            "operator (&+self[index: usize]): &+Element {\n    return &+self.values[index]\n}",
        ),
        (
            !has_expansion(MethodReceiverMode::ReadonlyBorrow),
            format!("operator (...&{owner}): Iterator"),
            "Declares readonly expansion for iteration and sequence spread.",
            "operator (...&self): Iterator {\n    return self.iter()\n}",
        ),
        (
            !has_expansion(MethodReceiverMode::ReadwriteBorrow),
            format!("operator (...&+{owner}): Iterator"),
            "Declares readwrite expansion for mutable iteration.",
            "operator (...&+self): Iterator {\n    return self.iter_mut()\n}",
        ),
        (
            !has_expansion(MethodReceiverMode::Owned),
            format!("operator (...{owner}): Iterator"),
            "Declares owned expansion for consuming iteration and sequence spread.",
            "operator (...self): Iterator {\n    return self.into_iter()\n}",
        ),
    ];
    for (_, detail, documentation, insert_text) in
        candidates.into_iter().filter(|(available, ..)| *available)
    {
        items.push(CompletionItemInfo {
            label: "operator".to_string(),
            kind: CompletionItemKind::Method,
            detail: Some(detail),
            documentation: Some(documentation.to_string()),
            insert_text: Some(insert_text.to_string()),
            sort_text: None,
            declaration_span: None,
        });
    }
}

fn associated_type_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let projection = facts.type_occurrences().find_map(|occurrence| {
        let TypeExpr::Projection(projection) = &occurrence.contextual_type else {
            return None;
        };
        (occurrence.focus_span.start <= offset && offset <= occurrence.focus_span.end)
            .then_some(projection)
    })?;
    let entries = match projection.base.as_ref() {
        TypeExpr::Reference(reference) if reference.name == "Self" => ast
            .items
            .iter()
            .find(|item| item.span().start <= offset && offset <= item.span().end)
            .and_then(|item| match item {
                Item::Interface(interface) => Some(
                    interface
                        .associated_types
                        .iter()
                        .map(|associated| {
                            (
                                associated.name.clone(),
                                interface.name.clone(),
                                associated.name_span,
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
                Item::Conformance(conformance) => {
                    let name = match &conformance.interface_ty {
                        TypeExpr::Reference(reference) => &reference.name,
                        TypeExpr::Generic(generic) => &generic.name,
                        _ => return None,
                    };
                    let interface = resolved.type_symbol_by_reference_name(name)?;
                    Some(
                        interface
                            .associated_types
                            .iter()
                            .map(|associated| {
                                (
                                    associated.name.clone(),
                                    interface.canonical_name.clone(),
                                    associated.name_span,
                                )
                            })
                            .collect(),
                    )
                }
                _ => None,
            })
            .unwrap_or_default(),
        TypeExpr::Reference(reference) => facts
            .generic_parameter_declarations()
            .find(|parameter| parameter.name == reference.name)
            .map(|parameter| {
                parameter
                    .bounds
                    .iter()
                    .filter_map(|bound| {
                        let name = match bound {
                            TypeExpr::Reference(reference) => &reference.name,
                            TypeExpr::Generic(generic) => &generic.name,
                            _ => return None,
                        };
                        resolved.type_symbol_by_reference_name(name)
                    })
                    .flat_map(|interface| {
                        interface.associated_types.iter().map(|associated| {
                            (
                                associated.name.clone(),
                                interface.canonical_name.clone(),
                                associated.name_span,
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                crate::typecheck::concrete_associated_types(&projection.base, resolved)
            }),
        _ => crate::typecheck::concrete_associated_types(&projection.base, resolved),
    };
    Some(
        entries
            .into_iter()
            .map(|(name, owner, declaration_span)| CompletionItemInfo {
                label: name.clone(),
                kind: CompletionItemKind::Class,
                detail: Some(format!("associated type {owner}.{name}")),
                documentation: None,
                insert_text: Some(name),
                sort_text: None,
                declaration_span: Some(declaration_span),
            })
            .collect(),
    )
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
            Item::Test(_) => {}
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
            Item::Instance(_) | Item::Conformance(_) => {
                for method in item.method_owner().expect("matched method owner").methods() {
                    if clause_contains_offset(method.result_provenance.as_ref(), offset) {
                        return Some(provenance_origin_items(
                            Some(method.receiver.mode),
                            &method.parameters.parameters,
                        ));
                    }
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    if clause_contains_offset(function.result_provenance.as_ref(), offset) {
                        return Some(provenance_origin_items(
                            None,
                            &function.parameters.parameters,
                        ));
                    }
                }
                for (_, literal) in construct.literals() {
                    if clause_contains_offset(literal.result_provenance.as_ref(), offset) {
                        return Some(provenance_origin_items(
                            None,
                            &literal.parameters.parameters,
                        ));
                    }
                }
            }
            Item::Destruct(_) => {}
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
        SymbolKind::Type(_) => {
            crate::analysis::presentation::type_declaration_presentation(symbol, resolved)
                .expect("type symbols have a declaration presentation")
                .render()
        }
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

fn symbol_insert_text(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => format!("{}()", symbol.name),
        SymbolKind::Type(_) | SymbolKind::Imported(_) => symbol.name.clone(),
    }
}

fn callable_detail(
    kind: &str,
    name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> String {
    crate::analysis::presentation::callable_signature_presentation(kind, name, signature, resolved)
        .render()
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
    owner
        .symbol
        .literals
        .iter()
        .filter(|literal| literal.is_accessible)
        .map(|literal| {
            let label = match literal.shape {
                LiteralShape::Sequence => "[]",
                LiteralShape::String => "\"\"",
            };
            CompletionItemInfo {
                label: label.to_string(),
                kind: CompletionItemKind::Constructor,
                detail: Some(
                    crate::analysis::presentation::literal_presentation_with_substitutions(
                        owner.symbol,
                        literal,
                        &owner.substitutions,
                        resolved,
                    )
                    .render(),
                ),
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
        TypeExpr::Callable(_) | TypeExpr::Closure(_) | TypeExpr::Opaque(_) => None,
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let self_ty = if symbol.generic_parameters.is_empty() {
                target.clone()
            } else {
                TypeExpr::Generic(crate::ast::GenericType {
                    span: reference.span,
                    name: reference.name.clone(),
                    name_span: reference.span,
                    arguments: symbol
                        .generic_parameters
                        .iter()
                        .map(|parameter| {
                            TypeExpr::Reference(crate::ast::TypeReference {
                                span: reference.span,
                                name: parameter.clone(),
                            })
                        })
                        .collect(),
                })
            };
            Some(ValueMemberOwner {
                symbol,
                substitutions: HashMap::from([("Self".to_string(), self_ty)]),
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
        TypeExpr::Projection(_) => None,
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
