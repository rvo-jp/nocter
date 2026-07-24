use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{FunctionDecl, ImplMember, Item, MethodDecl, TypeExpr};
use crate::source::ByteSpan;
use crate::typecheck::{FunctionCallSpecialization, MethodCallSpecialization};
use std::collections::{HashMap, VecDeque};

pub(crate) struct CallSpecializations {
    pub(crate) functions: HashMap<ByteSpan, Vec<FunctionCallSpecialization>>,
    pub(crate) methods: HashMap<ByteSpan, Vec<MethodCallSpecialization>>,
}

pub(crate) fn collect_call_specializations(analysis: &CompileUnitAnalysis) -> CallSpecializations {
    let mut functions: HashMap<ByteSpan, Vec<FunctionCallSpecialization>> = HashMap::new();
    let mut methods: HashMap<ByteSpan, Vec<MethodCallSpecialization>> = HashMap::new();
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
                enqueue_call_specializations_from_body(
                    file,
                    function.body.span,
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
                let Some(body) = method.body.as_ref() else {
                    continue;
                };
                enqueue_call_specializations_from_body(
                    file,
                    body.span,
                    &specialization.substitutions,
                    &mut queue,
                );
            }
        }
    }

    CallSpecializations { functions, methods }
}

enum PendingCallSpecialization {
    Function(FunctionCallSpecialization),
    Method(MethodCallSpecialization),
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

fn enqueue_call_specializations_from_body(
    file: &FileAnalysis,
    body_span: ByteSpan,
    context_substitutions: &HashMap<String, TypeExpr>,
    queue: &mut VecDeque<PendingCallSpecialization>,
) {
    for (call_span, specialization) in file.typecheck_facts.function_call_specialization_entries() {
        if !span_contains(body_span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Function(specialization));
        }
    }
    for (call_span, specialization) in file.typecheck_facts.method_call_specialization_entries() {
        if !span_contains(body_span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(PendingCallSpecialization::Method(specialization));
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

fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}
