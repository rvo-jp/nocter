use crate::abi::{
    AbiType, AbiValue, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis,
    call_specializations::{collect_call_specializations, impl_substitutions_for_self_ty},
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BindingStmt, Block, CallExpr, DropDecl, Expr, ForRangeStmt,
    FunctionDecl, IdentifierExpr, ImplDecl, ImplMember, InterpolatedStringPart, Item, MemberExpr,
    MethodDecl, OtherwiseExpr, Parameter, PayloadEnumPatternTargetShape, Stmt, StructLiteralField,
    SwitchPayloadPattern, TypeExpr, UnaryOperator, substitute_type_expr_parameters,
    type_expr_display_lossy,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::CallTarget;
use crate::literals::decode_integer_literal_value;
use crate::outcomes::outcome_shape_with_resolver;
use crate::resolve::{ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{
    FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts,
    TypecheckMethodReceiverKind, TypecheckPayloadBindingMode, TypecheckScalarViewKind,
    TypecheckSliceElementKind,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub(super) fn v0_buildability_diagnostics(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
) -> Vec<Diagnostic> {
    let Some(root) = analysis.root_file() else {
        return Vec::new();
    };

    let root_source = root.ast.span.source;
    let nocter_home = analysis.nocter_home.as_deref();
    let index = CallableIndex::new(analysis, root_source);
    let mut queue = VecDeque::from([CallTarget::same_file(DEFAULT_ENTRY_NAME)]);
    let mut seen = HashSet::new();
    let mut diagnostics = Vec::new();

    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }
        let Some(callable) = index.definition(&target) else {
            continue;
        };
        collect_callable_diagnostics(
            callable,
            sources,
            root_source,
            &index.names,
            &index.resolved_sources,
            nocter_home,
            &mut queue,
            &mut diagnostics,
        );
    }

    diagnostics
}

struct CallableIndex<'a> {
    definitions: HashMap<CallTarget, IndexedCallable<'a>>,
    names: HashMap<ByteSpan, String>,
    resolved_sources: ResolvedSources<'a>,
}

type ResolvedSources<'a> = HashMap<SourceId, &'a ResolveOutput>;

impl<'a> CallableIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        let mut names = HashMap::new();
        let call_specializations = collect_call_specializations(analysis);
        let resolved_sources = analysis
            .files
            .iter()
            .map(|file| (file.ast.span.source, &file.resolved))
            .collect::<ResolvedSources<'_>>();

        for file in &analysis.files {
            for item in &file.ast.items {
                match item {
                    Item::Function(function) if function.generics.parameters.is_empty() => {
                        let target = call_target_for_source(
                            file.ast.span.source,
                            root_source,
                            function.name.clone(),
                        );
                        names.insert(function.name_span, function.name.clone());
                        definitions.insert(
                            target,
                            IndexedCallable::new_function(
                                function,
                                file,
                                &resolved_sources,
                                root_source,
                            ),
                        );
                    }
                    Item::Function(function) => {
                        for specialization in call_specializations
                            .functions
                            .get(&function.name_span)
                            .or_else(|| {
                                call_specializations
                                    .functions
                                    .get(&function.member_name_span)
                            })
                            .into_iter()
                            .flatten()
                        {
                            let target = call_target_for_source(
                                file.ast.span.source,
                                root_source,
                                specialization.target_name.clone(),
                            );
                            definitions.insert(
                                target,
                                IndexedCallable::new_function_specialization(
                                    function,
                                    specialization.substitutions.clone(),
                                    file,
                                    &resolved_sources,
                                    root_source,
                                ),
                            );
                        }
                    }
                    Item::Impl(impl_) if impl_.interface_ty.is_none() => {
                        let Some(type_name) = impl_target_type_name(&impl_.target_ty) else {
                            continue;
                        };
                        for member in &impl_.members {
                            match member {
                                ImplMember::Method(method)
                                    if method.body.is_some()
                                        && impl_.generics.parameters.is_empty() =>
                                {
                                    let Some(body) = method.body.as_ref() else {
                                        continue;
                                    };
                                    let name = method_target_name(type_name, &method.name);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    names.insert(method.name_span, name.clone());
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_method(
                                            method,
                                            body,
                                            &impl_.target_ty,
                                            HashMap::new(),
                                            file,
                                            &resolved_sources,
                                        ),
                                    );
                                }
                                ImplMember::Method(method) if method.body.is_some() => {
                                    let Some(body) = method.body.as_ref() else {
                                        continue;
                                    };
                                    for specialization in call_specializations
                                        .methods
                                        .get(&method.name_span)
                                        .into_iter()
                                        .flatten()
                                    {
                                        let substitutions =
                                            method_specialization_context_substitutions(
                                                impl_,
                                                specialization,
                                            );
                                        let target = call_target_for_source(
                                            file.ast.span.source,
                                            root_source,
                                            specialization.target_name.clone(),
                                        );
                                        definitions.insert(
                                            target,
                                            IndexedCallable::new_method(
                                                method,
                                                body,
                                                &impl_.target_ty,
                                                substitutions,
                                                file,
                                                &resolved_sources,
                                            ),
                                        );
                                    }
                                }
                                ImplMember::Method(_) => {}
                                ImplMember::Drop(drop_) if impl_.generics.parameters.is_empty() => {
                                    let name = drop_target_name(&impl_.target_ty);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    names.insert(drop_name_span(drop_.span), name.clone());
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_drop(
                                            drop_,
                                            &impl_.target_ty,
                                            HashMap::new(),
                                            file,
                                            &resolved_sources,
                                        ),
                                    );
                                }
                                ImplMember::Drop(drop_) => {
                                    for specialization in call_specializations
                                        .drops
                                        .get(&drop_name_span(drop_.span))
                                        .into_iter()
                                        .flatten()
                                    {
                                        let target = call_target_for_source(
                                            file.ast.span.source,
                                            root_source,
                                            specialization.target_name.clone(),
                                        );
                                        definitions.insert(
                                            target,
                                            IndexedCallable::new_drop(
                                                drop_,
                                                &impl_.target_ty,
                                                specialization.substitutions.clone(),
                                                file,
                                                &resolved_sources,
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Item::Interface(interface) => {
                        for method in &interface.methods {
                            let Some(body) = &method.body else {
                                continue;
                            };
                            for specialization in call_specializations
                                .methods
                                .get(&method.name_span)
                                .into_iter()
                                .flatten()
                            {
                                let target = call_target_for_source(
                                    file.ast.span.source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        method,
                                        body,
                                        &specialization.self_ty,
                                        specialization.substitutions.clone(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Self {
            definitions,
            names,
            resolved_sources,
        }
    }

    fn definition(&self, target: &CallTarget) -> Option<&IndexedCallable<'a>> {
        self.definitions.get(target)
    }
}

struct IndexedCallable<'a> {
    span: ByteSpan,
    body: &'a Block,
    return_type: Option<TypeExpr>,
    substitutions: HashMap<String, TypeExpr>,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    issues: Vec<BuildabilityIssue>,
}

struct BuildabilityIssue {
    span: ByteSpan,
    construct: &'static str,
    help: &'static str,
}

impl<'a> IndexedCallable<'a> {
    fn new_function(
        function: &'a FunctionDecl,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
        root_source: SourceId,
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &HashMap::new(),
            &file.resolved,
            resolved_sources,
            root_source,
        ));
        issues.extend(unsupported_outcome_return_issue(
            function,
            &HashMap::new(),
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: function.span,
            body: &function.body,
            return_type: Some(function.return_type.clone()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_function_specialization(
        function: &'a FunctionDecl,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
        root_source: SourceId,
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &substitutions,
            &file.resolved,
            resolved_sources,
            root_source,
        ));
        issues.extend(unsupported_outcome_return_issue(
            function,
            &substitutions,
            &file.resolved,
            resolved_sources,
        ));
        let return_type = substitute_type_expr_parameters(&function.return_type, &substitutions);

        Self {
            span: function.span,
            body: &function.body,
            return_type: Some(return_type),
            substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_method(
        method: &'a MethodDecl,
        body: &'a Block,
        self_ty: &TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
    ) -> Self {
        let contextual_substitutions = method_contextual_substitutions(self_ty, &substitutions);
        let mut issues = Vec::new();
        issues.extend(callable_method_signature_issues(
            method,
            &contextual_substitutions,
            &file.resolved,
            resolved_sources,
        ));
        let return_type =
            substitute_type_expr_parameters(&method.return_type, &contextual_substitutions);
        issues.extend(unsupported_outcome_return_type_issue(
            &return_type,
            method.return_type.span(),
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: method.span,
            body,
            return_type: Some(return_type),
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_drop(
        drop_: &'a DropDecl,
        self_ty: &TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
    ) -> Self {
        let contextual_substitutions = method_contextual_substitutions(self_ty, &substitutions);
        let mut issues = Vec::new();
        issues.extend(callable_parameter_issues(
            std::slice::from_ref(&drop_.binding),
            &contextual_substitutions,
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: drop_.span,
            body: &drop_.body,
            return_type: None,
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }
}

mod diagnostics;
mod fixed_arrays;
mod runtime_support;
mod signatures;
mod statements;
mod traversal;
mod types;
mod variants;

use diagnostics::*;
use fixed_arrays::*;
use runtime_support::*;
use signatures::*;
use statements::*;
use traversal::*;
use types::*;
use variants::*;

#[cfg(test)]
mod tests;
