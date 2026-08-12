use super::literal_specializations::{LiteralSpecialization, collect_literal_specializations};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    DestructDecl, FunctionDecl, Item, MethodDecl, MethodOwnerDecl, TypeExpr, canonical_type_expr,
    substitute_type_expr_parameters,
};
use crate::semantic::{BodyId, DefId};
use crate::source::ByteSpan;
use crate::typecheck::{
    CallableCallSpecialization, DropTypeSpecialization, FunctionCallSpecialization,
    MethodCallSpecialization, TypecheckCoercionPlan,
};
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) struct CallSpecializations {
    pub(crate) functions: HashMap<DefId, Vec<FunctionCallSpecialization>>,
    pub(crate) callables: HashMap<BodyId, Vec<CallableCallSpecialization>>,
    pub(crate) methods: HashMap<DefId, Vec<MethodCallSpecialization>>,
    pub(crate) coercions: HashMap<DefId, Vec<TypecheckCoercionPlan>>,
    pub(crate) drops: HashMap<DefId, Vec<DropSpecialization>>,
    pub(crate) literals: HashMap<DefId, Vec<LiteralSpecialization>>,
    pub(crate) method_target_aliases: Vec<MethodTargetAlias>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodTargetAlias {
    pub(crate) requested_name: String,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DropSpecialization {
    pub(crate) def_id: DefId,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
}

pub(crate) fn collect_call_specializations(analysis: &CompileUnitAnalysis) -> CallSpecializations {
    let mut functions: HashMap<DefId, Vec<FunctionCallSpecialization>> = HashMap::new();
    let mut callables: HashMap<BodyId, Vec<CallableCallSpecialization>> = HashMap::new();
    let mut methods: HashMap<DefId, Vec<MethodCallSpecialization>> = HashMap::new();
    let mut coercions: HashMap<DefId, Vec<TypecheckCoercionPlan>> = HashMap::new();
    let mut drops: HashMap<DefId, Vec<DropSpecialization>> = HashMap::new();
    let mut method_target_aliases = Vec::new();
    let mut queue = VecDeque::new();
    let literals = collect_literal_specializations(analysis);

    for file in &analysis.files {
        for specialization in file.typed_hir.function_call_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
            {
                queue.push_back(PendingCallSpecialization::Function(specialization));
            }
        }
        for specialization in file.typed_hir.method_call_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
            {
                queue.push_back(PendingCallSpecialization::Method(specialization));
            }
        }
        for (_, plan) in file.typed_hir.coercion_plans() {
            if let Some(plan) = plan.with_context_substitutions(&HashMap::new()) {
                queue.push_back(PendingCallSpecialization::Coercion(plan));
            }
        }
        for (_, plan) in file.typed_hir.interpolation_plans() {
            for part in &plan.parts {
                let Some(specialization) =
                    protocol_method_call_specialization(analysis, &part.formatter)
                else {
                    continue;
                };
                if let Some(specialization) =
                    specialization.with_context_substitutions(&HashMap::new())
                {
                    queue.push_back(PendingCallSpecialization::Method(specialization));
                }
            }
        }
        for (_, fact) in file.typed_hir.callable_call_entries() {
            if let Some(specialization) = fact
                .specialization
                .with_context_substitutions(&HashMap::new())
            {
                queue.push_back(PendingCallSpecialization::Callable(specialization));
            }
        }
        for (_, plan) in file.typed_hir.collection_for_plans() {
            let Some(plan) = plan
                .with_context_substitutions(&HashMap::new())
                .and_then(|plan| {
                    crate::typecheck::specialize_collection_plan(plan, &file.resolved)
                })
            else {
                continue;
            };
            for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
                let Some(specialization) = protocol_method_call_specialization(analysis, method)
                else {
                    continue;
                };
                if let Some(specialization) =
                    specialization.with_context_substitutions(&HashMap::new())
                {
                    queue.push_back(PendingCallSpecialization::Method(specialization));
                }
            }
        }
        for (_, plan) in file.typed_hir.sequence_spread_plans() {
            let Some(plan) = plan
                .with_context_substitutions(&HashMap::new())
                .and_then(|plan| {
                    crate::typecheck::specialize_sequence_spread_plan(plan, &file.resolved)
                })
            else {
                continue;
            };
            for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
                let Some(specialization) = protocol_method_call_specialization(analysis, method)
                else {
                    continue;
                };
                if let Some(specialization) =
                    specialization.with_context_substitutions(&HashMap::new())
                {
                    queue.push_back(PendingCallSpecialization::Method(specialization));
                }
            }
        }
        for specialization in file.typed_hir.drop_type_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
                && let Some(specialization) =
                    drop_specialization_from_typecheck_fact(analysis, specialization)
            {
                queue.push_back(PendingCallSpecialization::Drop(specialization));
            }
        }
        for (_, ty) in file.typed_hir.binding_type_expr_entries() {
            enqueue_drop_dependencies_for_type(analysis, file, ty, &mut queue);
        }
    }

    for specializations in literals.values() {
        for specialization in specializations {
            let Some(file) = analysis.file_by_source(specialization.declaration_span.source) else {
                continue;
            };
            enqueue_call_specializations_from_span(
                analysis,
                file,
                specialization.declaration_span,
                &specialization.substitutions,
                &mut queue,
            );
        }
    }

    while let Some(specialization) = queue.pop_front() {
        match specialization {
            PendingCallSpecialization::Function(specialization) => {
                if !insert_function_specialization(&mut functions, specialization.clone()) {
                    continue;
                }
                let Some((file, function)) =
                    function_body_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                let mut context_substitutions = specialization.substitutions.clone();
                crate::typecheck::extend_associated_type_substitutions_with_resolver(
                    &mut context_substitutions,
                    &file.resolved,
                    |source| analysis.file_by_source(source).map(|file| &file.resolved),
                );
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    function.span,
                    &context_substitutions,
                    &mut queue,
                );
            }
            PendingCallSpecialization::Method(specialization) => {
                let requested_name = specialization.target_name.clone();
                let specialization =
                    redirect_interface_method_specialization(analysis, specialization);
                if requested_name != specialization.target_name {
                    method_target_aliases.push(MethodTargetAlias {
                        requested_name,
                        declaration_span: specialization.declaration_span,
                        target_name: specialization.target_name.clone(),
                    });
                }
                if !insert_method_specialization(&mut methods, specialization.clone()) {
                    continue;
                }
                let Some((file, owner, method)) =
                    method_body_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                if method.body.is_none() {
                    continue;
                }
                let context_substitutions = owner.map_or_else(
                    || {
                        let mut substitutions = specialization.substitutions.clone();
                        substitutions.insert("Self".to_string(), specialization.self_ty.clone());
                        crate::typecheck::extend_associated_type_substitutions_with_resolver(
                            &mut substitutions,
                            &file.resolved,
                            |source| analysis.file_by_source(source).map(|file| &file.resolved),
                        );
                        substitutions
                    },
                    |owner| {
                        method_specialization_context_substitutions(
                            owner,
                            &specialization,
                            &file.resolved,
                            analysis,
                        )
                    },
                );
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    method.span,
                    &context_substitutions,
                    &mut queue,
                );
            }
            PendingCallSpecialization::Callable(specialization) => {
                insert_callable_specialization(
                    &analysis.semantic_db,
                    &mut callables,
                    specialization,
                );
            }
            PendingCallSpecialization::Coercion(plan) => {
                let Some(plan) = crate::typecheck::specialize_coercion_plan_across_resolvers(
                    plan,
                    analysis.files.iter().map(|file| &file.resolved),
                ) else {
                    continue;
                };
                if !insert_coercion_specialization(&mut coercions, plan.clone()) {
                    continue;
                }
                let declaration_span = analysis
                    .semantic_db
                    .definition_span(plan.def_id.expect("specialized coercion identity"))
                    .expect("specialized coercion definition");
                let body_span = analysis
                    .callable_bodies
                    .implementation(declaration_span)
                    .unwrap_or(declaration_span);
                let Some(file) = analysis.file_by_source(body_span.source) else {
                    continue;
                };
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    body_span,
                    &plan.substitutions,
                    &mut queue,
                );
            }
            PendingCallSpecialization::Drop(specialization) => {
                if !insert_drop_specialization(&mut drops, specialization.clone()) {
                    continue;
                }
                let Some((file, drop_)) =
                    destruct_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    drop_.span,
                    &specialization.substitutions,
                    &mut queue,
                );
            }
        }
    }

    CallSpecializations {
        functions,
        callables,
        methods,
        coercions,
        drops,
        literals,
        method_target_aliases,
    }
}

fn redirect_interface_method_specialization(
    analysis: &CompileUnitAnalysis,
    mut specialization: MethodCallSpecialization,
) -> MethodCallSpecialization {
    let Some((interface_name, method_name)) =
        interface_method_identity_for_span(analysis, specialization.declaration_span)
    else {
        return specialization;
    };
    if matches!(specialization.self_ty, TypeExpr::Closure(_)) {
        return specialization;
    }
    let Some(file) = analysis
        .file_by_source(specialization.self_ty.span().source)
        .or_else(|| analysis.root_file())
    else {
        return specialization;
    };
    let Some(actual_method) = crate::typecheck::conformance_method_for_interface_type_expr(
        &specialization.self_ty,
        interface_name,
        method_name,
        &file.resolved,
    ) else {
        return specialization;
    };
    let Some((_file, Some(owner), _method)) =
        method_declaration_for_span(analysis, actual_method.name_span)
    else {
        return specialization;
    };
    let Some(owner_substitutions) =
        method_owner_substitutions_for_self_ty(owner, &specialization.self_ty)
    else {
        return specialization;
    };
    specialization.declaration_span = actual_method.name_span;
    specialization.def_id = analysis
        .semantic_db
        .definition_at(actual_method.name_span)
        .expect("resolved conformance method must have a semantic definition");
    let runtime_self_ty = substitute_type_expr_parameters(owner.target_ty(), &owner_substitutions);
    specialization.target_name = format!(
        "{}.{}",
        canonical_type_expr(&runtime_self_ty),
        specialization.method_name
    );
    specialization.substitutions.extend(owner_substitutions);
    specialization
}

fn interface_method_identity_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&str, &str)> {
    analysis.files.iter().find_map(|file| {
        file.resolved.symbols.symbols().find_map(|symbol| {
            let crate::resolve::SymbolKind::Type(interface) = &symbol.kind else {
                return None;
            };
            if interface.kind != crate::resolve::TypeSymbolKind::Interface {
                return None;
            }
            interface
                .methods
                .iter()
                .find(|method| method.name_span == declaration_span)
                .map(|method| (interface.canonical_name.as_str(), method.name.as_str()))
        })
    })
}

enum PendingCallSpecialization {
    Function(FunctionCallSpecialization),
    Callable(CallableCallSpecialization),
    Method(MethodCallSpecialization),
    Coercion(TypecheckCoercionPlan),
    Drop(DropSpecialization),
}

fn insert_coercion_specialization(
    specializations: &mut HashMap<DefId, Vec<TypecheckCoercionPlan>>,
    plan: TypecheckCoercionPlan,
) -> bool {
    let Some(def_id) = plan.def_id else {
        return false;
    };
    let entries = specializations.entry(def_id).or_default();
    if entries.iter().any(|entry| {
        type_expr_semantic_eq(&entry.self_ty, &plan.self_ty)
            && substitutions_semantic_eq(&entry.substitutions, &plan.substitutions)
    }) {
        return false;
    }
    entries.push(plan);
    true
}

fn insert_callable_specialization(
    semantic_db: &crate::semantic::SemanticDb,
    specializations: &mut HashMap<BodyId, Vec<CallableCallSpecialization>>,
    specialization: CallableCallSpecialization,
) -> bool {
    let Some(body_id) = semantic_db.body_at(specialization.callable_ty.span()) else {
        return false;
    };
    let entries = specializations.entry(body_id).or_default();
    if entries.iter().any(|entry| {
        entry.target_name == specialization.target_name
            && entry.callable_ty == specialization.callable_ty
            && entry.capability == specialization.capability
    }) {
        return false;
    }
    entries.push(specialization);
    true
}

fn insert_function_specialization(
    specializations: &mut HashMap<DefId, Vec<FunctionCallSpecialization>>,
    specialization: FunctionCallSpecialization,
) -> bool {
    let entries = specializations.entry(specialization.def_id).or_default();
    if entries.iter().any(|entry| {
        entry.target_name == specialization.target_name
            && entry.substitutions == specialization.substitutions
    }) {
        return false;
    }
    entries.push(specialization);
    true
}

fn insert_method_specialization(
    specializations: &mut HashMap<DefId, Vec<MethodCallSpecialization>>,
    specialization: MethodCallSpecialization,
) -> bool {
    let entries = specializations.entry(specialization.def_id).or_default();
    if entries.iter().any(|entry| {
        entry.target_name == specialization.target_name
            && entry.self_ty == specialization.self_ty
            && entry.substitutions == specialization.substitutions
    }) {
        return false;
    }
    entries.push(specialization);
    true
}

fn insert_drop_specialization(
    specializations: &mut HashMap<DefId, Vec<DropSpecialization>>,
    specialization: DropSpecialization,
) -> bool {
    let entries = specializations.entry(specialization.def_id).or_default();
    if entries.iter().any(|entry| {
        entry.target_name == specialization.target_name
            && type_expr_semantic_eq(&entry.self_ty, &specialization.self_ty)
            && substitutions_semantic_eq(&entry.substitutions, &specialization.substitutions)
    }) {
        return false;
    }
    entries.push(specialization);
    true
}

fn enqueue_call_specializations_from_span(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    span: ByteSpan,
    context_substitutions: &HashMap<String, TypeExpr>,
    queue: &mut VecDeque<PendingCallSpecialization>,
) {
    for (binding_span, ty) in file.typed_hir.binding_type_expr_entries() {
        if !span_contains(span, binding_span) {
            continue;
        }
        let ty = substitute_type_expr_parameters(ty, context_substitutions);
        enqueue_drop_dependencies_for_type(analysis, file, &ty, queue);
    }
    for (call_span, specialization) in file.typed_hir.function_call_specialization_entries() {
        if !span_contains(span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Function(specialization));
        }
    }
    for (call_span, specialization) in file.typed_hir.method_call_specialization_entries() {
        if !span_contains(span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Method(specialization));
        }
    }
    for (expression_span, plan) in file.typed_hir.coercion_plans() {
        if !span_contains(span, expression_span) {
            continue;
        }
        if let Some(plan) = plan.with_context_substitutions(context_substitutions)
            && let Some(plan) = crate::typecheck::specialize_coercion_plan_across_resolvers(
                plan,
                analysis.files.iter().map(|candidate| &candidate.resolved),
            )
        {
            queue.push_back(PendingCallSpecialization::Coercion(plan));
        }
    }
    for (call_span, fact) in file.typed_hir.callable_call_entries() {
        if !span_contains(span, call_span) {
            continue;
        }
        if let Some(specialization) = fact
            .specialization
            .with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Callable(specialization));
        }
    }
    for specialization in file.typed_hir.drop_type_specializations() {
        if !span_contains(span, specialization.self_ty.span()) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
            && let Some(specialization) =
                drop_specialization_from_typecheck_fact(analysis, specialization)
        {
            queue.push_back(PendingCallSpecialization::Drop(specialization));
        }
    }
    for plan in file.typed_hir.index_plans() {
        if !span_contains(span, plan.expression_span) {
            continue;
        }
        let Some(plan) = plan.with_context_substitutions(context_substitutions) else {
            continue;
        };
        let selected = crate::typecheck::specialize_index_plan_across_resolvers(
            plan,
            analysis.files.iter().map(|candidate| &candidate.resolved),
        );
        let Some(method) = selected.and_then(|selected| selected.method) else {
            continue;
        };
        let Some(specialization) = protocol_method_call_specialization(analysis, &method) else {
            continue;
        };
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Method(specialization));
        }
    }
    for (expression_span, plan) in file.typed_hir.interpolation_plans() {
        if !span_contains(span, expression_span) {
            continue;
        }
        for part in &plan.parts {
            let Some(specialization) =
                protocol_method_call_specialization(analysis, &part.formatter)
            else {
                continue;
            };
            if let Some(specialization) =
                specialization.with_context_substitutions(context_substitutions)
            {
                queue.push_back(PendingCallSpecialization::Method(specialization));
            }
        }
    }
    for (statement_span, plan) in file.typed_hir.collection_for_plans() {
        if !span_contains(span, *statement_span) {
            continue;
        }
        let Some(plan) = plan
            .with_context_substitutions(context_substitutions)
            .and_then(|plan| crate::typecheck::specialize_collection_plan(plan, &file.resolved))
        else {
            continue;
        };
        for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
            let Some(specialization) = protocol_method_call_specialization(analysis, method) else {
                continue;
            };
            if let Some(specialization) =
                specialization.with_context_substitutions(context_substitutions)
            {
                queue.push_back(PendingCallSpecialization::Method(specialization));
            }
        }
    }
    for (spread_span, plan) in file.typed_hir.sequence_spread_plans() {
        if !span_contains(span, *spread_span) {
            continue;
        }
        let Some(plan) = plan
            .with_context_substitutions(context_substitutions)
            .and_then(|plan| {
                crate::typecheck::specialize_sequence_spread_plan(plan, &file.resolved)
            })
        else {
            continue;
        };
        for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
            let Some(specialization) = protocol_method_call_specialization(analysis, method) else {
                continue;
            };
            if let Some(specialization) =
                specialization.with_context_substitutions(context_substitutions)
            {
                queue.push_back(PendingCallSpecialization::Method(specialization));
            }
        }
    }
}

fn enqueue_drop_dependencies_for_type(
    analysis: &CompileUnitAnalysis,
    fallback_file: &FileAnalysis,
    ty: &TypeExpr,
    queue: &mut VecDeque<PendingCallSpecialization>,
) {
    for specialization in
        super::drop_dependencies::concrete_drop_dependencies(analysis, fallback_file, ty)
    {
        if let Some(specialization) =
            drop_specialization_from_typecheck_fact(analysis, specialization)
        {
            queue.push_back(PendingCallSpecialization::Drop(specialization));
        }
    }
}

fn protocol_method_call_specialization(
    analysis: &CompileUnitAnalysis,
    method: &crate::typecheck::TypecheckProtocolMethod,
) -> Option<MethodCallSpecialization> {
    let Some((_file, Some(owner), _declaration)) =
        method_declaration_for_span(analysis, method.declaration_span)
    else {
        return interface_method_identity_for_span(analysis, method.declaration_span)
            .is_some()
            .then(|| method.as_method_call_specialization(Vec::new(), HashMap::new()));
    };
    let substitutions = method_owner_substitutions_for_self_ty(owner, &method.self_ty)?;
    let generic_parameters = owner
        .generics()
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect();
    Some(method.as_method_call_specialization(generic_parameters, substitutions))
}

fn function_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, &FunctionDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| {
            let function = match item {
                Item::Function(function)
                    if function.name_span == declaration_span
                        || function.member_name_span == declaration_span =>
                {
                    Some(function)
                }
                Item::Construct(construct) => construct.functions().find_map(|(_, function)| {
                    (function.name_span == declaration_span
                        || function.member_name_span == declaration_span)
                        .then_some(function)
                }),
                _ => None,
            }?;
            Some((file, function))
        })
    })
}

fn function_body_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, &FunctionDecl)> {
    let body_span = analysis
        .callable_bodies
        .implementation(declaration_span)
        .unwrap_or(declaration_span);
    function_declaration_for_span(analysis, body_span)
}

fn method_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, Option<&dyn MethodOwnerDecl>, &MethodDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| match item {
            Item::Instance(_) | Item::Conformance(_) => item.method_owner().and_then(|owner| {
                owner.methods().find_map(|method| {
                    (method.name_span == declaration_span).then_some((file, Some(owner), method))
                })
            }),
            Item::Interface(interface) => interface.methods.iter().find_map(|method| {
                (method.name_span == declaration_span).then_some((file, None, method))
            }),
            _ => None,
        })
    })
}

fn method_body_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, Option<&dyn MethodOwnerDecl>, &MethodDecl)> {
    let body_span = analysis
        .callable_bodies
        .implementation(declaration_span)
        .unwrap_or(declaration_span);
    method_declaration_for_span(analysis, body_span)
}

fn destruct_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, &DestructDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| {
            let Item::Destruct(destruct) = item else {
                return None;
            };
            (destruct.keyword_span == declaration_span).then_some((file, destruct))
        })
    })
}

fn drop_specialization_from_typecheck_fact(
    analysis: &CompileUnitAnalysis,
    specialization: DropTypeSpecialization,
) -> Option<DropSpecialization> {
    let (_file, destruct) =
        destruct_declaration_for_span(analysis, specialization.declaration_span)?;
    let substitutions = declaration_pattern_substitutions_for_self_ty(
        &destruct.generics,
        &destruct.target_ty,
        &specialization.self_ty,
    )?;
    let self_ty = substitute_type_expr_parameters(&destruct.target_ty, &substitutions);
    Some(DropSpecialization {
        def_id: specialization.def_id,
        declaration_span: specialization.declaration_span,
        target_name: drop_target_name(&self_ty),
        self_ty,
        substitutions,
    })
}

fn method_specialization_context_substitutions(
    owner: &(impl MethodOwnerDecl + ?Sized),
    specialization: &MethodCallSpecialization,
    resolved: &crate::resolve::ResolveOutput,
    analysis: &CompileUnitAnalysis,
) -> HashMap<String, TypeExpr> {
    let mut substitutions =
        method_owner_substitutions_for_self_ty(owner, &specialization.self_ty).unwrap_or_default();
    substitutions.extend(specialization.substitutions.clone());
    crate::typecheck::extend_associated_type_substitutions_with_resolver(
        &mut substitutions,
        resolved,
        |source| analysis.file_by_source(source).map(|file| &file.resolved),
    );
    substitutions
}

pub(crate) fn method_owner_substitutions_for_self_ty(
    owner: &(impl MethodOwnerDecl + ?Sized),
    self_ty: &TypeExpr,
) -> Option<HashMap<String, TypeExpr>> {
    declaration_pattern_substitutions_for_self_ty(owner.generics(), owner.target_ty(), self_ty)
}

fn declaration_pattern_substitutions_for_self_ty(
    generics: &crate::ast::GenericParamList,
    target_ty: &TypeExpr,
    self_ty: &TypeExpr,
) -> Option<HashMap<String, TypeExpr>> {
    let generic_parameters = generics
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_owner_substitutions(target_ty, self_ty, &generic_parameters, &mut substitutions)
        .then_some(substitutions)
}

fn infer_owner_substitutions(
    expected: &TypeExpr,
    actual: &TypeExpr,
    generic_parameters: &HashSet<String>,
    substitutions: &mut HashMap<String, TypeExpr>,
) -> bool {
    match expected {
        TypeExpr::Callable(expected) => {
            let TypeExpr::Callable(actual) = actual else {
                return false;
            };
            expected.capability == actual.capability
                && expected.parameters.len() == actual.parameters.len()
                && expected
                    .parameters
                    .iter()
                    .zip(&actual.parameters)
                    .all(|(expected, actual)| {
                        infer_owner_substitutions(
                            &expected.ty,
                            &actual.ty,
                            generic_parameters,
                            substitutions,
                        )
                    })
                && infer_owner_substitutions(
                    &expected.return_type,
                    &actual.return_type,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::Closure(expected) => {
            matches!(actual, TypeExpr::Closure(actual) if expected.span == actual.span)
        }
        TypeExpr::Opaque(expected) => {
            matches!(actual, TypeExpr::Opaque(actual) if expected.some_span == actual.some_span)
        }
        TypeExpr::Reference(reference) if generic_parameters.contains(&reference.name) => {
            insert_owner_substitution(&reference.name, actual, substitutions)
        }
        TypeExpr::Reference(expected) => match actual {
            TypeExpr::Reference(actual) => type_names_match(&expected.name, &actual.name),
            _ => false,
        },
        TypeExpr::Generic(expected) => {
            let TypeExpr::Generic(actual) = actual else {
                return false;
            };
            type_names_match(&expected.name, &actual.name)
                && expected.arguments.len() == actual.arguments.len()
                && expected.arguments.iter().zip(actual.arguments.iter()).all(
                    |(expected, actual)| {
                        infer_owner_substitutions(
                            expected,
                            actual,
                            generic_parameters,
                            substitutions,
                        )
                    },
                )
        }
        TypeExpr::Projection(expected) => {
            let TypeExpr::Projection(actual) = actual else {
                return false;
            };
            expected.name == actual.name
                && infer_owner_substitutions(
                    &expected.base,
                    &actual.base,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::Pointer(expected) => {
            let TypeExpr::Pointer(actual) = actual else {
                return false;
            };
            infer_owner_substitutions(
                &expected.inner,
                &actual.inner,
                generic_parameters,
                substitutions,
            )
        }
        TypeExpr::Borrow(expected) => {
            let TypeExpr::Borrow(actual) = actual else {
                return false;
            };
            expected.is_readwrite == actual.is_readwrite
                && infer_owner_substitutions(
                    &expected.inner,
                    &actual.inner,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::View(expected) => {
            let TypeExpr::View(actual) = actual else {
                return false;
            };
            expected.is_readwrite == actual.is_readwrite
                && infer_owner_substitutions(
                    &expected.element,
                    &actual.element,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::Array(expected) => {
            let TypeExpr::Array(actual) = actual else {
                return false;
            };
            expected.length.value == actual.length.value
                && infer_owner_substitutions(
                    &expected.element,
                    &actual.element,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::Optional(expected) => {
            let TypeExpr::Optional(actual) = actual else {
                return false;
            };
            infer_owner_substitutions(
                &expected.inner,
                &actual.inner,
                generic_parameters,
                substitutions,
            )
        }
        TypeExpr::Fallible(expected) => {
            let TypeExpr::Fallible(actual) = actual else {
                return false;
            };
            infer_owner_substitutions(
                &expected.success,
                &actual.success,
                generic_parameters,
                substitutions,
            ) && infer_owner_substitutions(
                &expected.error,
                &actual.error,
                generic_parameters,
                substitutions,
            )
        }
    }
}

fn insert_owner_substitution(
    name: &str,
    ty: &TypeExpr,
    substitutions: &mut HashMap<String, TypeExpr>,
) -> bool {
    match substitutions.get(name) {
        Some(existing) => type_expr_semantic_eq(existing, ty),
        None => {
            substitutions.insert(name.to_string(), ty.clone());
            true
        }
    }
}

fn substitutions_semantic_eq(
    left: &HashMap<String, TypeExpr>,
    right: &HashMap<String, TypeExpr>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(name, left_ty)| {
            right
                .get(name)
                .is_some_and(|right_ty| type_expr_semantic_eq(left_ty, right_ty))
        })
}

fn type_expr_semantic_eq(left: &TypeExpr, right: &TypeExpr) -> bool {
    canonical_type_expr(left) == canonical_type_expr(right)
}

fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", canonical_type_expr(self_ty))
}

fn type_names_match(left: &str, right: &str) -> bool {
    left == right || short_type_name(left) == short_type_name(right)
}

fn short_type_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn keys_closure_specializations_by_body_identity() {
        let (_sources, analysis) = analyze_text(
            r#"func apply<F>(callback: F): i32 where F: &func(i32): i32 {
    return callback(3)
}

func main(): i32 {
    return apply((value) { value * 2 })
}
"#,
        );
        let specializations = collect_call_specializations(&analysis);
        let (body, entries) = specializations
            .callables
            .iter()
            .next()
            .expect("closure specialization");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            analysis.semantic_db.body_at(entries[0].callable_ty.span()),
            Some(*body)
        );
    }
}
