use super::literal_specializations::{LiteralSpecialization, collect_literal_specializations};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    DropDecl, FunctionDecl, ImplDecl, ImplMember, Item, MethodDecl, TypeExpr,
    substitute_type_expr_parameters, type_expr_display_lossy,
};
use crate::source::ByteSpan;
use crate::typecheck::{
    DropTypeSpecialization, FunctionCallSpecialization, MethodCallSpecialization,
};
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) struct CallSpecializations {
    pub(crate) functions: HashMap<ByteSpan, Vec<FunctionCallSpecialization>>,
    pub(crate) methods: HashMap<ByteSpan, Vec<MethodCallSpecialization>>,
    pub(crate) drops: HashMap<ByteSpan, Vec<DropSpecialization>>,
    pub(crate) literals: HashMap<ByteSpan, Vec<LiteralSpecialization>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DropSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
}

pub(crate) fn collect_call_specializations(analysis: &CompileUnitAnalysis) -> CallSpecializations {
    let mut functions: HashMap<ByteSpan, Vec<FunctionCallSpecialization>> = HashMap::new();
    let mut methods: HashMap<ByteSpan, Vec<MethodCallSpecialization>> = HashMap::new();
    let mut drops: HashMap<ByteSpan, Vec<DropSpecialization>> = HashMap::new();
    let mut queue = VecDeque::new();
    let literals = collect_literal_specializations(analysis);

    for file in &analysis.files {
        for specialization in file.typecheck_facts.function_call_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
            {
                queue.push_back(PendingCallSpecialization::Function(specialization));
            }
        }
        for specialization in file.typecheck_facts.method_call_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
            {
                queue.push_back(PendingCallSpecialization::Method(specialization));
            }
        }
        for (_, plan) in file.typecheck_facts.collection_for_plans() {
            for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
                let Some(specialization) = iteration_method_call_specialization(analysis, method)
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
        for (_, plan) in file.typecheck_facts.sequence_spread_plans() {
            for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
                let Some(specialization) = iteration_method_call_specialization(analysis, method)
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
        for specialization in file.typecheck_facts.drop_type_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
                && let Some(specialization) =
                    drop_specialization_from_typecheck_fact(analysis, specialization)
            {
                queue.push_back(PendingCallSpecialization::Drop(specialization));
            }
        }
        for (_, ty) in file.typecheck_facts.binding_type_expr_entries() {
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
                    function_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    function.span,
                    &specialization.substitutions,
                    &mut queue,
                );
            }
            PendingCallSpecialization::Method(specialization) => {
                let specialization =
                    redirect_interface_method_specialization(analysis, specialization);
                if !insert_method_specialization(&mut methods, specialization.clone()) {
                    continue;
                }
                let Some((file, impl_, method)) =
                    method_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                if method.body.is_none() {
                    continue;
                }
                let context_substitutions = impl_.map_or_else(
                    || {
                        let mut substitutions = specialization.substitutions.clone();
                        substitutions.insert("Self".to_string(), specialization.self_ty.clone());
                        substitutions
                    },
                    |impl_| method_specialization_context_substitutions(impl_, &specialization),
                );
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    method.span,
                    &context_substitutions,
                    &mut queue,
                );
            }
            PendingCallSpecialization::Drop(specialization) => {
                if !insert_drop_specialization(&mut drops, specialization.clone()) {
                    continue;
                }
                let Some((file, _impl_, drop_)) =
                    drop_declaration_for_span(analysis, specialization.declaration_span)
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
        methods,
        drops,
        literals,
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
    let Some(actual_method) = crate::typecheck::implementation_for_interface_type_expr(
        &specialization.self_ty,
        interface_name,
        method_name,
        &file.resolved,
    ) else {
        return specialization;
    };
    specialization.declaration_span = actual_method.name_span;
    if let Some((_file, Some(impl_), _method)) =
        method_declaration_for_span(analysis, actual_method.name_span)
        && let Some(impl_substitutions) =
            impl_substitutions_for_self_ty(impl_, &specialization.self_ty)
    {
        specialization.substitutions.extend(impl_substitutions);
    }
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
    Method(MethodCallSpecialization),
    Drop(DropSpecialization),
}

fn insert_function_specialization(
    specializations: &mut HashMap<ByteSpan, Vec<FunctionCallSpecialization>>,
    specialization: FunctionCallSpecialization,
) -> bool {
    let entries = specializations
        .entry(specialization.declaration_span)
        .or_default();
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
    specializations: &mut HashMap<ByteSpan, Vec<MethodCallSpecialization>>,
    specialization: MethodCallSpecialization,
) -> bool {
    let entries = specializations
        .entry(specialization.declaration_span)
        .or_default();
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
    specializations: &mut HashMap<ByteSpan, Vec<DropSpecialization>>,
    specialization: DropSpecialization,
) -> bool {
    let entries = specializations
        .entry(specialization.declaration_span)
        .or_default();
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
    for (binding_span, ty) in file.typecheck_facts.binding_type_expr_entries() {
        if !span_contains(span, binding_span) {
            continue;
        }
        let ty = substitute_type_expr_parameters(ty, context_substitutions);
        enqueue_drop_dependencies_for_type(analysis, file, &ty, queue);
    }
    for (call_span, specialization) in file.typecheck_facts.function_call_specialization_entries() {
        if !span_contains(span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Function(specialization));
        }
    }
    for (call_span, specialization) in file.typecheck_facts.method_call_specialization_entries() {
        if !span_contains(span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Method(specialization));
        }
    }
    for specialization in file.typecheck_facts.drop_type_specializations() {
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
    for (statement_span, plan) in file.typecheck_facts.collection_for_plans() {
        if !span_contains(span, *statement_span) {
            continue;
        }
        for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
            let Some(specialization) = iteration_method_call_specialization(analysis, method)
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
    for (spread_span, plan) in file.typecheck_facts.sequence_spread_plans() {
        if !span_contains(span, *spread_span) {
            continue;
        }
        for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
            let Some(specialization) = iteration_method_call_specialization(analysis, method)
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

fn iteration_method_call_specialization(
    analysis: &CompileUnitAnalysis,
    method: &crate::typecheck::TypecheckIterationMethod,
) -> Option<MethodCallSpecialization> {
    let Some((_file, Some(impl_), _declaration)) =
        method_declaration_for_span(analysis, method.declaration_span)
    else {
        return interface_method_identity_for_span(analysis, method.declaration_span)
            .is_some()
            .then(|| method.as_method_call_specialization(Vec::new(), HashMap::new()));
    };
    let substitutions = impl_substitutions_for_self_ty(impl_, &method.self_ty)?;
    let generic_parameters = impl_
        .generics
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
            let Item::Function(function) = item else {
                return None;
            };
            (function.name_span == declaration_span
                || function.member_name_span == declaration_span)
                .then_some((file, function))
        })
    })
}

fn method_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, Option<&ImplDecl>, &MethodDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| match item {
            Item::Impl(impl_) => impl_.members.iter().find_map(|member| {
                let ImplMember::Method(method) = member else {
                    return None;
                };
                (method.name_span == declaration_span).then_some((file, Some(impl_), method))
            }),
            Item::Interface(interface) => interface.methods.iter().find_map(|method| {
                (method.name_span == declaration_span).then_some((file, None, method))
            }),
            _ => None,
        })
    })
}

fn drop_declaration_for_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&FileAnalysis, &ImplDecl, &DropDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| {
            let Item::Impl(impl_) = item else {
                return None;
            };
            impl_.members.iter().find_map(|member| {
                let ImplMember::Drop(drop_) = member else {
                    return None;
                };
                (drop_.name_span == declaration_span).then_some((file, impl_, drop_))
            })
        })
    })
}

fn drop_specialization_from_typecheck_fact(
    analysis: &CompileUnitAnalysis,
    specialization: DropTypeSpecialization,
) -> Option<DropSpecialization> {
    let (_file, impl_, _drop_) =
        drop_declaration_for_span(analysis, specialization.declaration_span)?;
    let substitutions = impl_substitutions_for_self_ty(impl_, &specialization.self_ty)?;
    let self_ty = substitute_type_expr_parameters(&impl_.target_ty, &substitutions);
    Some(DropSpecialization {
        declaration_span: specialization.declaration_span,
        target_name: drop_target_name(&self_ty),
        self_ty,
        substitutions,
    })
}

fn method_specialization_context_substitutions(
    impl_: &ImplDecl,
    specialization: &MethodCallSpecialization,
) -> HashMap<String, TypeExpr> {
    let mut substitutions =
        impl_substitutions_for_self_ty(impl_, &specialization.self_ty).unwrap_or_default();
    substitutions.extend(specialization.substitutions.clone());
    substitutions
}

pub(crate) fn impl_substitutions_for_self_ty(
    impl_: &ImplDecl,
    self_ty: &TypeExpr,
) -> Option<HashMap<String, TypeExpr>> {
    let generic_parameters = impl_
        .generics
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_impl_substitutions(
        &impl_.target_ty,
        self_ty,
        &generic_parameters,
        &mut substitutions,
    )
    .then_some(substitutions)
}

fn infer_impl_substitutions(
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
                        infer_impl_substitutions(
                            &expected.ty,
                            &actual.ty,
                            generic_parameters,
                            substitutions,
                        )
                    })
                && infer_impl_substitutions(
                    &expected.return_type,
                    &actual.return_type,
                    generic_parameters,
                    substitutions,
                )
        }
        TypeExpr::Closure(expected) => {
            matches!(actual, TypeExpr::Closure(actual) if expected.span == actual.span)
        }
        TypeExpr::Reference(reference) if generic_parameters.contains(&reference.name) => {
            insert_impl_substitution(&reference.name, actual, substitutions)
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
                        infer_impl_substitutions(
                            expected,
                            actual,
                            generic_parameters,
                            substitutions,
                        )
                    },
                )
        }
        TypeExpr::Pointer(expected) => {
            let TypeExpr::Pointer(actual) = actual else {
                return false;
            };
            infer_impl_substitutions(
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
                && infer_impl_substitutions(
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
                && infer_impl_substitutions(
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
                && infer_impl_substitutions(
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
            infer_impl_substitutions(
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
            infer_impl_substitutions(
                &expected.success,
                &actual.success,
                generic_parameters,
                substitutions,
            ) && infer_impl_substitutions(
                &expected.error,
                &actual.error,
                generic_parameters,
                substitutions,
            )
        }
    }
}

fn insert_impl_substitution(
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
    type_expr_display_lossy(left) == type_expr_display_lossy(right)
}

fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", type_expr_display_lossy(self_ty))
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
