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
        for specialization in file.typecheck_facts.drop_type_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
                && let Some(specialization) =
                    drop_specialization_from_typecheck_fact(analysis, specialization)
            {
                queue.push_back(PendingCallSpecialization::Drop(specialization));
            }
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
                if !insert_method_specialization(&mut methods, specialization.clone()) {
                    continue;
                }
                let Some((file, method)) =
                    method_declaration_for_span(analysis, specialization.declaration_span)
                else {
                    continue;
                };
                if method.body.is_none() {
                    continue;
                }
                enqueue_call_specializations_from_span(
                    analysis,
                    file,
                    method.span,
                    &specialization.substitutions,
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
    }
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
}

fn function_declaration_for_span<'a>(
    analysis: &'a CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&'a FileAnalysis, &'a FunctionDecl)> {
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

fn method_declaration_for_span<'a>(
    analysis: &'a CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&'a FileAnalysis, &'a MethodDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| {
            let Item::Impl(impl_) = item else {
                return None;
            };
            impl_.members.iter().find_map(|member| {
                let ImplMember::Method(method) = member else {
                    return None;
                };
                (method.name_span == declaration_span).then_some((file, method))
            })
        })
    })
}

fn drop_declaration_for_span<'a>(
    analysis: &'a CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<(&'a FileAnalysis, &'a ImplDecl, &'a DropDecl)> {
    analysis.files.iter().find_map(|file| {
        file.ast.items.iter().find_map(|item| {
            let Item::Impl(impl_) = item else {
                return None;
            };
            impl_.members.iter().find_map(|member| {
                let ImplMember::Drop(drop_) = member else {
                    return None;
                };
                (drop_name_span(drop_.span) == declaration_span).then_some((file, impl_, drop_))
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

fn impl_substitutions_for_self_ty(
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

fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
}
