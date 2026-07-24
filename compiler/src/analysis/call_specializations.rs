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
    let methods = method_specializations(analysis);
    let functions = function_specializations(analysis, &methods);
    CallSpecializations { functions, methods }
}

fn function_specializations(
    analysis: &CompileUnitAnalysis,
    method_specializations: &HashMap<ByteSpan, Vec<MethodCallSpecialization>>,
) -> HashMap<ByteSpan, Vec<FunctionCallSpecialization>> {
    let mut specializations: HashMap<ByteSpan, Vec<FunctionCallSpecialization>> = HashMap::new();
    let mut queue = VecDeque::new();
    for file in &analysis.files {
        for specialization in file.typecheck_facts.function_call_specializations() {
            if let Some(specialization) = specialization.with_context_substitutions(&HashMap::new())
            {
                queue.push_back(specialization);
            }
        }
    }
    for (method_span, specializations) in method_specializations {
        let Some((file, method)) = method_declaration_for_span(analysis, *method_span) else {
            continue;
        };
        let Some(body) = method.body.as_ref() else {
            continue;
        };
        for method_specialization in specializations {
            enqueue_function_specializations_from_body(
                file,
                body.span,
                &method_specialization.substitutions,
                &mut queue,
            );
        }
    }

    while let Some(specialization) = queue.pop_front() {
        if !insert_function_specialization(&mut specializations, specialization.clone()) {
            continue;
        }
        let Some((file, function)) =
            function_declaration_for_span(analysis, specialization.declaration_span)
        else {
            continue;
        };
        enqueue_function_specializations_from_body(
            file,
            function.body.span,
            &specialization.substitutions,
            &mut queue,
        );
    }

    specializations
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

fn enqueue_function_specializations_from_body(
    file: &FileAnalysis,
    body_span: ByteSpan,
    context_substitutions: &HashMap<String, TypeExpr>,
    queue: &mut VecDeque<FunctionCallSpecialization>,
) {
    for (call_span, specialization) in file.typecheck_facts.function_call_specialization_entries() {
        if !span_contains(body_span, call_span) {
            continue;
        }
        if let Some(specialization) =
            specialization.with_context_substitutions(context_substitutions)
        {
            queue.push_back(specialization);
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

fn method_specializations(
    analysis: &CompileUnitAnalysis,
) -> HashMap<ByteSpan, Vec<MethodCallSpecialization>> {
    let mut specializations: HashMap<ByteSpan, Vec<MethodCallSpecialization>> = HashMap::new();
    for file in &analysis.files {
        for specialization in file.typecheck_facts.method_call_specializations() {
            let entries = specializations
                .entry(specialization.declaration_span)
                .or_default();
            if !entries.iter().any(|entry| {
                entry.target_name == specialization.target_name
                    && entry.self_ty == specialization.self_ty
                    && entry.substitutions == specialization.substitutions
            }) {
                entries.push(specialization.clone());
            }
        }
    }
    specializations
}

fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}
