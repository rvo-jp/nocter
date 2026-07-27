use crate::abi::{AbiType, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis,
    call_specializations::{collect_call_specializations, impl_substitutions_for_self_ty},
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BindingStmt, Block, CallExpr, DropDecl, Expr, ForRangeStmt,
    FunctionDecl, ImplDecl, ImplMember, Item, MemberExpr, MethodDecl, Parameter, Stmt, TypeExpr,
    substitute_type_expr_parameters, type_expr_display_lossy,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::CallTarget;
use crate::literals::decode_integer_literal_value;
use crate::resolve::{ResolveOutput, SymbolKind, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{
    FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts, TypecheckScalarViewKind,
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
                            IndexedCallable::new_function(function, file, &resolved_sources),
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
                    _ => {}
                }
            }
        }

        Self { definitions, names }
    }

    fn definition(&self, target: &CallTarget) -> Option<&IndexedCallable<'a>> {
        self.definitions.get(target)
    }
}

struct IndexedCallable<'a> {
    span: ByteSpan,
    body: &'a Block,
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
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &HashMap::new(),
            &file.resolved,
            resolved_sources,
        ));
        issues.extend(nested_fallible_return_issue(function, &file.resolved));

        Self {
            span: function.span,
            body: &function.body,
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
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &substitutions,
            &file.resolved,
            resolved_sources,
        ));
        issues.extend(nested_fallible_return_issue(function, &file.resolved));

        Self {
            span: function.span,
            body: &function.body,
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
        issues.extend(nested_fallible_return_type_issue(
            &method.return_type,
            &file.resolved,
        ));

        Self {
            span: method.span,
            body,
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
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }
}

fn callable_function_signature_issues(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let mut issues = callable_parameter_issues(
        &function.parameters.parameters,
        substitutions,
        resolved,
        resolved_sources,
    );
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(&return_type, resolved, &source_resolver) {
        issues.push(BuildabilityIssue {
            span: function.return_type.span(),
            construct: "function return types outside the v0 runtime ABI subset",
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, `error`, an aggregate with a non-empty ABI layout, or a fallible form of one of those types",
        });
    }
    issues
}

fn callable_method_signature_issues(
    method: &MethodDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let mut issues = callable_parameter_issues(
        std::slice::from_ref(&method.receiver),
        substitutions,
        resolved,
        resolved_sources,
    );
    issues.extend(callable_parameter_issues(
        &method.parameters.parameters,
        substitutions,
        resolved,
        resolved_sources,
    ));
    let return_type = substitute_type_expr_parameters(&method.return_type, substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(&return_type, resolved, &source_resolver) {
        issues.push(BuildabilityIssue {
            span: method.return_type.span(),
            construct: "method return types outside the v0 runtime ABI subset",
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, `error`, an aggregate with a non-empty ABI layout, or a fallible form of one of those types",
        });
    }
    issues
}

fn callable_parameter_issues(
    parameters: &[Parameter],
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    parameters
        .iter()
        .filter_map(|parameter| {
            let ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
            if callable_parameter_type_is_buildable_with_resolver(&ty, resolved, &source_resolver) {
                return None;
            }
            Some(BuildabilityIssue {
                span: parameter.span,
                construct: "function or method parameters outside the v0 runtime ABI subset",
                help: "use `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `error`, scalar borrow parameters, aggregate borrow parameters, or aggregate value parameters with non-empty ABI layouts",
            })
        })
        .collect()
}

fn method_contextual_substitutions(
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> HashMap<String, TypeExpr> {
    let concrete_self_ty = substitute_type_expr_parameters(self_ty, substitutions);
    let mut contextual = substitutions.clone();
    contextual.insert("Self".to_string(), concrete_self_ty);
    contextual
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

fn collect_callable_diagnostics(
    callable: &IndexedCallable<'_>,
    sources: &SourceMap,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for issue in &callable.issues {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            issue.span,
            issue.construct,
            issue.help,
        ));
    }

    enqueue_drop_targets_in_callable(callable, root_source, queue);

    collect_terminal_return_block_diagnostics(
        callable.body,
        sources,
        callable.resolved,
        callable.typecheck_facts,
        &callable.substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn enqueue_drop_targets_in_callable(
    callable: &IndexedCallable<'_>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    for specialization in callable.typecheck_facts.drop_type_specializations() {
        if !span_contains(callable.span, specialization.self_ty.span()) {
            continue;
        }
        let Some(specialization) =
            specialization.with_context_substitutions(&callable.substitutions)
        else {
            continue;
        };
        queue.push_back(call_target_for_source(
            specialization.declaration_span.source,
            root_source,
            specialization.target_name,
        ));
    }
}

fn collect_terminal_return_block_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = &block.result {
        collect_terminal_return_expression_diagnostics(
            result,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_block_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = &block.result {
        collect_expression_diagnostics(
            result,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_terminal_return_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_is_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_match_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::If(expression) if terminal_if_expression_is_buildable(expression) => {
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::IfIs(expression) if terminal_if_is_expression_is_buildable(expression, resolved) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Match(expression) if terminal_match_expression_is_buildable(expression, resolved) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_terminal_return_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(else_arm) = &expression.else_arm {
                collect_terminal_return_block_diagnostics(
                    &else_arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        _ => collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
    }
}

fn collect_value_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression) if value_if_expression_is_buildable(expression) => {
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::IfIs(expression) if value_if_is_expression_is_buildable(expression, resolved) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Match(expression) if value_match_expression_is_buildable(expression, resolved) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_value_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(else_arm) = &expression.else_arm {
                collect_value_block_diagnostics(
                    &else_arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        _ => collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
    }
}

fn collect_value_block_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(result) = &block.result else {
        return;
    };
    collect_value_expression_diagnostics(
        result,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_void_effect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_is_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_match_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        _ => {
            if let Some(diagnostic) = unsupported_expression_statement_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
    }
}

fn collect_void_effect_if_expression_diagnostics(
    expression: &crate::ast::IfStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression_diagnostics(
        &expression.condition,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_void_effect_block_diagnostics(
        &expression.then_block,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
    if let Some(else_block) = &expression.else_block {
        collect_void_effect_block_diagnostics(
            else_block,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_void_effect_if_is_expression_diagnostics(
    expression: &crate::ast::IfIsStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression_diagnostics(
        &expression.expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_void_effect_block_diagnostics(
        &expression.then_block,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
    if let Some(else_block) = &expression.else_block {
        collect_void_effect_block_diagnostics(
            else_block,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_void_effect_match_expression_diagnostics(
    expression: &crate::ast::SwitchStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression_diagnostics(
        &expression.expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        nocter_home,
        queue,
        diagnostics,
    );
    for arm in &expression.arms {
        collect_void_effect_block_diagnostics(
            &arm.body,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(else_arm) = &expression.else_arm {
        collect_void_effect_block_diagnostics(
            &else_arm.body,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_void_effect_block_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = &block.result {
        collect_void_effect_expression_diagnostics(
            result,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn binding_initializer_may_use_value_control_expression(
    statement: &crate::ast::BindingStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    if typecheck_facts
        .binding_scalar_view_kind(statement.name_span)
        .is_some()
    {
        return true;
    }

    let Some(ty) = &statement.ty else {
        return false;
    };
    type_expr_is_buildable_scalar_or_view(ty, resolved)
}

fn assignment_value_may_use_value_control_expression(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            typecheck_facts
                .binding_scalar_view_kind(symbol.name_span)
                .is_some()
        }
        Expr::Member(member) => typecheck_facts
            .field_scalar_view_kind(member.member_span)
            .is_some_and(field_kind_may_use_value_control_expression),
        _ => false,
    }
}

fn call_argument_may_use_value_control_expression(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
    .is_some_and(|ty| type_expr_is_buildable_scalar_or_view(&ty, resolved))
}

fn call_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref()
        && let Some(ty) = method_call_argument_parameter_type(
            member,
            index,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    {
        return Some(ty);
    }

    let signature = resolved.call_signature_for_call(call)?;
    let parameter = signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
    }

    ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    Some(ty)
}

fn method_call_argument_parameter_type(
    member: &crate::ast::MemberExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let method_name_span = typecheck_facts.method_call_target(member.member_span)?;
    let method = resolved.method_signature_by_name_span(method_name_span)?;
    let parameter = method.signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
    {
        let self_substitution =
            HashMap::from([("Self".to_string(), specialization.self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
        return Some(substitute_type_expr_parameters(&ty, generic_substitutions));
    }

    if typecheck_facts
        .generic_method_call_target(member.member_span)
        .is_some()
    {
        return None;
    }

    if let Some(self_ty) = &method.impl_target_ty {
        let self_substitution = HashMap::from([("Self".to_string(), self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
    }
    Some(substitute_type_expr_parameters(&ty, generic_substitutions))
}

fn struct_literal_field_may_use_value_control_expression(
    field_name_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    typecheck_facts
        .field_scalar_view_kind(field_name_span)
        .is_some_and(field_kind_may_use_value_control_expression)
}

fn field_kind_may_use_value_control_expression(kind: TypecheckScalarViewKind) -> bool {
    match kind {
        TypecheckScalarViewKind::I32
        | TypecheckScalarViewKind::U8
        | TypecheckScalarViewKind::Usize
        | TypecheckScalarViewKind::Bool
        | TypecheckScalarViewKind::Str => true,
        TypecheckScalarViewKind::Slice(element) => {
            typecheck_slice_element_kind_is_buildable(element)
        }
    }
}

fn unsupported_local_binding_type_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if local_binding_type_is_buildable(statement, resolved, typecheck_facts, generic_substitutions)
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        statement.name_span,
        "local bindings with unsupported value types",
        "bind `i32`, `u8`, `usize`, `bool`, `&str`, slice views, payloadless enums, errors, or aggregate values until broader scalar local lowering is promoted",
    ))
}

fn local_binding_type_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if let Some(ty) = &statement.ty {
        let ty = substitute_type_expr_parameters(ty, generic_substitutions);
        return local_binding_type_expr_is_buildable(&ty, resolved)
            || !type_expr_is_known_unsupported_scalar_value(&ty, resolved);
    }

    if typecheck_facts
        .binding_scalar_view_kind(statement.name_span)
        .is_some()
    {
        return true;
    }

    typecheck_facts
        .binding_type_label(statement.name_span)
        .map_or(true, |label| {
            inferred_binding_type_label_is_buildable(label, resolved)
        })
}

fn inferred_binding_type_label_is_buildable(label: &str, resolved: &ResolveOutput) -> bool {
    if unsupported_scalar_type_label(label) {
        return false;
    }

    let Some(symbol) = resolved.type_symbol_by_reference_name(label) else {
        return true;
    };
    let Some(target) = &symbol.alias_target else {
        return true;
    };
    !type_expr_is_known_unsupported_scalar_value(target, resolved)
}

fn unsupported_scalar_type_label(label: &str) -> bool {
    matches!(
        label,
        "i8" | "i16" | "i64" | "isize" | "u16" | "u32" | "u64"
    )
}

fn local_binding_type_expr_is_buildable(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_buildable_scalar_or_view(ty, resolved)
        || type_expr_is_error_parameter(ty, resolved)
        || type_expr_is_supported_aggregate_value(ty, resolved)
}

fn type_expr_is_known_unsupported_scalar_value(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_known_unsupported_scalar_value_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_is_known_unsupported_scalar_value_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Reference(reference) if unsupported_scalar_type_label(&reference.name) => true,
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_known_unsupported_scalar_value_inner(
                target,
                resolved,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => abi_value_from_type_expr(ty, resolved).is_ok_and(|value| {
            matches!(
                value.ty,
                AbiType::I8
                    | AbiType::I16
                    | AbiType::I64
                    | AbiType::Isize
                    | AbiType::U16
                    | AbiType::U32
                    | AbiType::U64
            )
        }),
    }
}

fn resolved_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> &'a ResolveOutput
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    resolver(ty.span().source).unwrap_or(fallback_resolved)
}

fn type_expr_is_buildable_scalar_or_view(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, resolved, &|_| Some(resolved))
}

fn type_expr_is_buildable_scalar_or_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_buildable_scalar_or_view_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn type_expr_is_buildable_scalar_or_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference)
            if matches!(reference.name.as_str(), "i32" | "u8" | "usize" | "bool") =>
        {
            true
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            true
        }
        TypeExpr::Borrow(borrow)
            if type_expr_resolves_to_view_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            ) =>
        {
            true
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return type_expr_has_buildable_scalar_abi_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return type_expr_has_buildable_scalar_abi_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_buildable_scalar_or_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => type_expr_has_buildable_scalar_abi_with_resolver(ty, fallback_resolved, resolver),
    }
}

fn type_expr_has_buildable_scalar_abi_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, |source| resolver(source))
        .is_ok_and(|value| {
            matches!(
                value.ty,
                AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::Pointer
            )
        })
}

fn type_expr_resolves_to_str(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_resolves_to_str_with_resolver(ty, resolved, &|_| Some(resolved))
}

fn type_expr_resolves_to_str_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_builtin_reference_inner(
        ty,
        fallback_resolved,
        resolver,
        "str",
        &mut HashSet::new(),
    )
}

fn type_expr_resolves_to_builtin_reference_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    expected: &str,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == expected => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_resolves_to_builtin_reference_inner(
                target,
                fallback_resolved,
                resolver,
                expected,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}

fn type_expr_resolves_to_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_view_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_resolves_to_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(_) => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_resolves_to_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}

fn type_expr_resolves_to_supported_slice_view(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<bool> {
    type_expr_resolves_to_supported_slice_view_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_resolves_to_supported_slice_view_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<bool> {
    match ty {
        TypeExpr::View(view) => Some(type_expr_is_supported_slice_index_element(
            &view.element,
            resolved,
        )),
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result =
                type_expr_resolves_to_supported_slice_view_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn type_expr_resolved_view_element_kind(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<TypecheckSliceElementKind> {
    type_expr_resolved_view_element_kind_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_resolved_view_element_kind_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind> {
    match ty {
        TypeExpr::View(view) => Some(type_expr_slice_element_kind(&view.element, resolved)),
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result =
                type_expr_resolved_view_element_kind_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn callable_parameter_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    callable_parameter_type_is_buildable_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn callable_parameter_type_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return callable_non_alias_parameter_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return callable_non_alias_parameter_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = callable_parameter_type_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => callable_non_alias_parameter_type_is_buildable_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ),
    }
}

fn callable_non_alias_parameter_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_error_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_borrow_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
}

fn callable_return_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    callable_return_type_is_buildable_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn callable_return_type_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if matches!(reference.name.as_str(), "void" | "never") => {
            true
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return callable_non_alias_return_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return callable_non_alias_return_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = callable_return_type_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => callable_return_type_is_buildable_inner(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => callable_return_type_is_buildable_inner(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        _ => callable_non_alias_return_type_is_buildable_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ),
    }
}

fn callable_non_alias_return_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_error_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
}

fn type_expr_is_error_parameter(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_error_parameter_with_resolver(ty, resolved, &|_| Some(resolved))
}

fn type_expr_is_error_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_error_parameter_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_is_error_parameter_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "error" => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_error_parameter_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}

fn type_expr_is_supported_borrow_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let TypeExpr::Borrow(borrow) = ty else {
        return false;
    };
    if !borrow.is_readwrite
        && type_expr_resolves_to_str_with_resolver(&borrow.inner, fallback_resolved, resolver)
    {
        return true;
    }
    if type_expr_resolves_to_view_with_resolver(&borrow.inner, fallback_resolved, resolver) {
        return true;
    }
    abi_value_from_type_expr_with_resolver(&borrow.inner, fallback_resolved, |source| {
        resolver(source)
    })
    .is_ok_and(|value| {
        matches!(
            value.ty,
            AbiType::I32
                | AbiType::U8
                | AbiType::Usize
                | AbiType::Bool
                | AbiType::Pointer
                | AbiType::Struct(_)
        )
    })
}

fn type_expr_is_supported_aggregate_value(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_supported_aggregate_value_with_resolver(ty, resolved, &|_| Some(resolved))
}

fn type_expr_is_supported_aggregate_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, |source| resolver(source))
        .is_ok_and(|value| matches!(value.ty, AbiType::Struct(_)) && value.layout.size > 0)
}

fn value_if_expression_is_buildable(expression: &crate::ast::IfStmt) -> bool {
    expression.else_block.is_some()
        && value_block_is_expression_only(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_expression_only)
}

fn value_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
) -> bool {
    terminal_if_is_expression_is_buildable(expression, resolved)
        && value_block_is_expression_only(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_expression_only)
}

fn value_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    terminal_match_expression_is_buildable(expression, resolved)
        && expression
            .arms
            .iter()
            .all(|arm| value_block_is_expression_only(&arm.body))
        && expression
            .else_arm
            .as_ref()
            .map_or(true, |arm| value_block_is_expression_only(&arm.body))
}

fn value_block_is_expression_only(block: &Block) -> bool {
    block.statements.is_empty() && block.result.is_some()
}

fn void_effect_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && expression.else_block.as_ref().map_or(true, |block| {
        void_effect_block_is_buildable(block, resolved, typecheck_facts, generic_substitutions)
    })
}

fn void_effect_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if_is_statement_is_buildable(expression, resolved)
        && void_effect_block_is_buildable(
            &expression.then_block,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
        && expression.else_block.as_ref().map_or(true, |block| {
            void_effect_block_is_buildable(block, resolved, typecheck_facts, generic_substitutions)
        })
}

fn void_effect_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    switch_statement_is_buildable(expression, resolved)
        && expression.arms.iter().all(|arm| {
            void_effect_block_is_buildable(
                &arm.body,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        })
        && expression.else_arm.as_ref().map_or(true, |arm| {
            void_effect_block_is_buildable(
                &arm.body,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        })
}

fn void_effect_block_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match block.result.as_deref() {
        Some(result) => void_effect_expression_is_buildable(
            result,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        None => true,
    }
}

fn void_effect_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(expression) => void_effect_if_expression_is_buildable(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(expression) => void_effect_if_is_expression_is_buildable(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(expression) => void_effect_match_expression_is_buildable(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => expression_statement_is_supported(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
    }
}

fn terminal_if_expression_is_buildable(expression: &crate::ast::IfStmt) -> bool {
    expression.else_block.is_some()
}

fn terminal_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
) -> bool {
    expression.else_block.is_some() && if_is_statement_is_buildable(expression, resolved)
}

fn terminal_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    switch_statement_is_buildable(expression, resolved)
        && (expression.else_arm.is_some()
            || switch_statement_covers_all_payloadless_variants(expression, resolved))
}

fn if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
) -> bool {
    if statement.payload.is_some() {
        return false;
    }

    let Some(symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        return false;
    };
    if symbol.kind != TypeSymbolKind::Enum
        || symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    let Some(index) = symbol
        .variants
        .iter()
        .position(|variant| variant.name == statement.variant_name)
    else {
        return false;
    };
    u8::try_from(index).is_ok()
}

fn switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return false;
    };
    if statement.arms.iter().any(|arm| arm.payload.is_some()) {
        return false;
    }

    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        resolved
            .type_symbol_by_name(&arm.enum_name)
            .is_some_and(|symbol| symbol.canonical_name == target_symbol.canonical_name)
            && target_symbol
                .variants
                .iter()
                .any(|variant| variant.name == arm.variant_name)
    })
}

fn switch_statement_covers_all_payloadless_variants(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return false;
    };
    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    target_symbol.variants.iter().all(|variant| {
        statement.arms.iter().any(|arm| {
            arm.enum_name == first_arm.enum_name
                && arm.payload.is_none()
                && arm.variant_name == variant.name
        })
    })
}

fn collect_statement_diagnostics(
    statement: &Stmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_terminal_return_expression_diagnostics(
                    expression,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Binding(statement) => {
            if let Some(diagnostic) = unsupported_local_binding_type_diagnostic(
                sources,
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if binding_initializer_may_use_value_control_expression(
                statement,
                resolved,
                typecheck_facts,
            ) {
                collect_value_expression_diagnostics(
                    &statement.initializer,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &statement.initializer,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Assignment(statement) => {
            enqueue_member_replacement_drop_target(
                statement,
                typecheck_facts,
                generic_substitutions,
                root_source,
                queue,
            );
            if !assignment_operator_is_buildable(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.operator_span,
                    "compound assignment statements",
                    "use `i32`, `usize`, or `u8` whole-binding, aggregate-field, or read-write slice element compound assignment, or use `target = target op value` until broader compound assignment lowering is promoted",
                ));
            }
            if let Some(diagnostic) = unsupported_index_assignment_target_diagnostic(
                sources,
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &statement.target,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if assignment_value_may_use_value_control_expression(
                statement,
                resolved,
                typecheck_facts,
            ) {
                collect_value_expression_diagnostics(
                    &statement.value,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &statement.value,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::If(statement) => {
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::IfIs(statement) => {
            if !if_is_statement_is_buildable(statement, resolved) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.pattern_span,
                    "`if is` pattern branches",
                    "use payloadless enum patterns, or keep payload pattern code on the `check` path",
                ));
            }
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Switch(statement) => {
            if !switch_statement_is_buildable(statement, resolved) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "`match` statements",
                    "use payloadless enum `match` arms, or keep payload pattern code on the `check` path",
                ));
            }
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &statement.arms {
                collect_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::ForRange(statement) => {
            if !range_for_binding_type_is_buildable(statement, typecheck_facts) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.range_span,
                    "range `for` loops outside i32/usize bounds",
                    "use `i32` or `usize` bounds, or use `while` with explicit scalar state until broader range `for` lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &statement.start,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.end,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::While(statement) => {
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Loop(statement) => {
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Expression(statement) => {
            if let Some(diagnostic) = unsupported_expression_statement_diagnostic(
                sources,
                &statement.expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn enqueue_member_replacement_drop_target(
    statement: &AssignmentStmt,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    if statement.operator != AssignmentOperator::Assign {
        return;
    }
    let Expr::Member(member) = unwrap_group_expr(&statement.target) else {
        return;
    };
    let Some(specialization) = typecheck_facts.field_drop_type_specialization(member.member_span)
    else {
        return;
    };
    let Some(specialization) = specialization.with_context_substitutions(generic_substitutions)
    else {
        return;
    };
    queue.push_back(call_target_for_source(
        specialization.declaration_span.source,
        root_source,
        specialization.target_name,
    ));
}

fn unsupported_expression_statement_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if expression_statement_is_supported(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span(),
        "value-producing expression statements",
        "call a void, never, or discardable scalar/view/aggregate function, handle a discardable scalar/view/aggregate fallible call with `?`, `!`, or `catch`, or bind/return the value explicitly",
    ))
}

fn expression_statement_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape(call, resolved, typecheck_facts, generic_substitutions) {
                Some(
                    ReturnShape::Void
                    | ReturnShape::Never
                    | ReturnShape::DiscardableScalar
                    | ReturnShape::DiscardableView
                    | ReturnShape::DiscardableAggregate,
                )
                | None => true,
                Some(ReturnShape::FallibleDiscardable | ReturnShape::Other) => false,
            }
        }
        Expr::Propagate(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::StructLiteral(literal) => aggregate_literal_statement_is_supported(literal, resolved),
        _ => false,
    }
}

fn aggregate_literal_statement_is_supported(
    literal: &crate::ast::StructLiteralExpr,
    resolved: &ResolveOutput,
) -> bool {
    abi_value_from_type_expr(&literal.ty, resolved)
        .map(|value| matches!(value.ty, AbiType::Struct(_)))
        .unwrap_or(false)
}

fn fallible_void_statement_inner_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape(call, resolved, typecheck_facts, generic_substitutions) {
                Some(ReturnShape::FallibleDiscardable) | None => true,
                Some(
                    ReturnShape::Void
                    | ReturnShape::Never
                    | ReturnShape::DiscardableScalar
                    | ReturnShape::DiscardableView
                    | ReturnShape::DiscardableAggregate
                    | ReturnShape::Other,
                ) => false,
            }
        }
        _ => false,
    }
}

fn range_for_binding_type_is_buildable(
    statement: &ForRangeStmt,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.binding_type_label(statement.name_span),
        Some("i32" | "usize")
    )
}

fn assignment_operator_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator == AssignmentOperator::Assign {
        return true;
    }
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            matches!(
                typecheck_facts.binding_type_label(symbol.name_span),
                Some("i32" | "usize" | "u8")
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typecheck_facts)
        }
        Expr::Index(index) => slice_index_compound_assignment_is_buildable(
            &index.object,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn slice_index_compound_assignment_is_buildable(
    object: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    matches!(
        slice_index_assignment_element_kind(
            object,
            resolved,
            typecheck_facts,
            generic_substitutions
        ),
        Some(
            TypecheckSliceElementKind::I32
                | TypecheckSliceElementKind::U8
                | TypecheckSliceElementKind::Usize,
        )
    )
}

fn unsupported_index_assignment_target_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    let Expr::Index(index) = unwrap_group_expr(&statement.target) else {
        return None;
    };
    if matches!(
        slice_index_assignment_target_is_buildable(
            &index.object,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Some(true) | None
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        index.object.span(),
        "index assignment targets outside supported slice values",
        "assign through a slice binding, supported slice-returning call result, or slice aggregate field until broader index assignment lowering is promoted",
    ))
}

fn slice_index_assignment_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Slice(element) => {
                    typecheck_slice_element_kind_is_buildable(element).then_some(true)
                }
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool
                | TypecheckScalarViewKind::Str => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_is_buildable(&return_type, resolved)
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Slice(element) => {
                typecheck_slice_element_kind_is_buildable(element).then_some(true)
            }
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool
            | TypecheckScalarViewKind::Str => None,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_target_is_buildable(
            &propagation.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_target_is_buildable(
            &force.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_target_is_buildable(
            &catch.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_target_is_buildable(
            &group.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn slice_index_assignment_fallible_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    let Expr::Call(call) = unwrap_group_expr(expression) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    slice_index_target_type_expr_is_buildable(&fallible.success, resolved)
}

fn aggregate_field_compound_assignment_is_buildable(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.field_scalar_view_kind(member_span),
        Some(
            TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize,
        )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnShape {
    Void,
    Never,
    DiscardableScalar,
    DiscardableView,
    DiscardableAggregate,
    FallibleDiscardable,
    Other,
}

fn call_return_shape(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ReturnShape> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    Some(return_shape_from_type_expr(&return_type, resolved))
}

fn return_shape_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> ReturnShape {
    return_shape_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn return_shape_from_type_expr_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> ReturnShape {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => ReturnShape::Void,
        TypeExpr::Reference(reference) if reference.name == "never" => ReturnShape::Never,
        TypeExpr::Reference(reference)
            if matches!(reference.name.as_str(), "i32" | "u8" | "usize" | "bool") =>
        {
            ReturnShape::DiscardableScalar
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite && type_expr_resolves_to_str(&borrow.inner, resolved) =>
        {
            ReturnShape::DiscardableView
        }
        TypeExpr::Borrow(borrow)
            if type_expr_resolves_to_supported_slice_view(&borrow.inner, resolved)
                .unwrap_or(false) =>
        {
            ReturnShape::DiscardableView
        }
        _ if type_expr_is_supported_aggregate_return(ty, resolved) => {
            ReturnShape::DiscardableAggregate
        }
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return ReturnShape::Other;
            };
            let Some(target) = &symbol.alias_target else {
                return ReturnShape::Other;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return ReturnShape::Other;
            }
            let shape = return_shape_from_type_expr_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            shape
        }
        TypeExpr::Fallible(fallible) => {
            match return_shape_from_type_expr_inner(&fallible.success, resolved, resolving_names) {
                ReturnShape::Void
                | ReturnShape::DiscardableScalar
                | ReturnShape::DiscardableView
                | ReturnShape::DiscardableAggregate => ReturnShape::FallibleDiscardable,
                ReturnShape::Never | ReturnShape::FallibleDiscardable | ReturnShape::Other => {
                    ReturnShape::Other
                }
            }
        }
        _ => ReturnShape::Other,
    }
}

fn type_expr_is_supported_aggregate_return(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    let Ok(value) = abi_value_from_type_expr(ty, resolved) else {
        return false;
    };
    matches!(value.ty, AbiType::Struct(_)) && value.layout.size > 0
}

fn collect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::InterpolatedString(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "bare string interpolation",
                "construct `String` explicitly with an allocator and `std/fmt.append_*` calls",
            ));
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_expression_diagnostics(
                        &part.expression,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "array literals",
                "use scalar/view values or a std collection API once v0 array storage is promoted",
            ));
            for element in &expression.elements {
                collect_expression_diagnostics(
                    element,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                if struct_literal_field_may_use_value_control_expression(
                    field.name_span,
                    typecheck_facts,
                ) {
                    collect_value_expression_diagnostics(
                        &field.value,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else {
                    collect_expression_diagnostics(
                        &field.value,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                }
            }
        }
        Expr::Propagate(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Force(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Catch(expression) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.catch_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Unary(expression) => collect_expression_diagnostics(
            &expression.operand,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Binary(expression) => {
            collect_expression_diagnostics(
                &expression.left,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.right,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::TypeConversion(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Call(expression) => {
            let check_only_std_call = unsupported_check_only_std_call_diagnostic(
                sources,
                expression,
                resolved,
                nocter_home,
            );
            if let Some(diagnostic) = &check_only_std_call {
                diagnostics.push(diagnostic.clone());
            }
            let unsupported_std_vec_element_call = unsupported_std_vec_element_call_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                nocter_home,
            );
            if let Some(diagnostic) = &unsupported_std_vec_element_call {
                diagnostics.push(diagnostic.clone());
            }
            if let Some(diagnostic) = unsupported_null_from_addr_call_diagnostic(
                sources,
                expression,
                resolved,
                nocter_home,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) =
                unsupported_unloaded_imported_call_diagnostic(sources, expression, resolved)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_borrow_call_argument_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_method_borrow_receiver_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_unspecialized_generic_function_call_diagnostic(
                sources,
                expression,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_unspecialized_generic_method_call_diagnostic(
                sources,
                expression,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &expression.callee,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if check_only_std_call.is_none()
                && unsupported_std_vec_element_call.is_none()
                && let Some(target) = call_target_for_call(
                    expression,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                )
            {
                queue.push_back(target);
            }
            for (index, argument) in expression.arguments.iter().enumerate() {
                if call_argument_may_use_value_control_expression(
                    expression,
                    index,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_value_expression_diagnostics(
                        argument,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else {
                    collect_expression_diagnostics(
                        argument,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                }
            }
        }
        Expr::Member(expression) => {
            if let Some(diagnostic) = unsupported_payload_enum_value_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_field_member_value_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &expression.object,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Index(expression) => {
            if let Some(diagnostic) = unsupported_slice_index_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                nocter_home,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &expression.object,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.index,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Group(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Otherwise(expression) => {
            collect_expression_diagnostics(
                &expression.value,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.fallback,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::If(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`if` expressions",
                "use an explicit `if` statement with `return` until backend expression lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::IfIs(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`if is` expressions",
                "use an explicit `if is` statement with `return` until backend expression lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Match(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`match` expressions",
                "use an explicit `match` statement with `return` until backend expression lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(else_arm) = &expression.else_arm {
                collect_block_diagnostics(
                    &else_arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
    }
}

fn unsupported_field_member_value_diagnostic(
    sources: &SourceMap,
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if typecheck_facts
        .field_scalar_view_kind(expression.member_span)
        .is_some()
    {
        return None;
    }

    let field_ty = field_type_expr_for_member(expression, resolved, typecheck_facts)?;
    let field_ty = substitute_type_expr_parameters(field_ty, generic_substitutions);
    match member_field_value_type_is_buildable(&field_ty, resolved)? {
        true => None,
        false => Some(unsupported_v0_build_diagnostic(
            sources,
            expression.member_span,
            "field member values outside supported scalar/view or aggregate types",
            "keep `u16`, `u32`, and other storage-only fields encapsulated in aggregates, or expose an `i32`, `usize`, or `u8` value until broader scalar field lowering is promoted",
        )),
    }
}

fn field_type_expr_for_member<'a>(
    expression: &MemberExpr,
    resolved: &'a ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<&'a TypeExpr> {
    let target_span = typecheck_facts.field_target(expression.member_span)?;
    resolved.symbols.symbols().find_map(|symbol| {
        let SymbolKind::Type(type_symbol) = &symbol.kind else {
            return None;
        };
        type_symbol
            .fields
            .iter()
            .find(|field| field.name_span == target_span)
            .map(|field| &field.ty)
    })
}

fn member_field_value_type_is_buildable(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<bool> {
    if type_expr_contains_unresolved_type_parameter(ty, resolved) {
        return None;
    }
    if type_expr_is_buildable_scalar_or_view(ty, resolved)
        || type_expr_is_supported_aggregate_value(ty, resolved)
    {
        return Some(true);
    }
    Some(false)
}

fn type_expr_contains_unresolved_type_parameter(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    match ty {
        TypeExpr::Reference(reference) => {
            !known_builtin_type_name(&reference.name)
                && resolved
                    .type_symbol_by_reference_name(&reference.name)
                    .is_none()
        }
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|argument| type_expr_contains_unresolved_type_parameter(argument, resolved)),
        TypeExpr::Pointer(pointer) => {
            type_expr_contains_unresolved_type_parameter(&pointer.inner, resolved)
        }
        TypeExpr::Borrow(borrow) => {
            type_expr_contains_unresolved_type_parameter(&borrow.inner, resolved)
        }
        TypeExpr::View(view) => {
            type_expr_contains_unresolved_type_parameter(&view.element, resolved)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_unresolved_type_parameter(&array.element, resolved)
        }
        TypeExpr::Optional(optional) => {
            type_expr_contains_unresolved_type_parameter(&optional.inner, resolved)
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_unresolved_type_parameter(&fallible.success, resolved)
                || type_expr_contains_unresolved_type_parameter(&fallible.error, resolved)
        }
    }
}

fn known_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "void"
            | "never"
            | "bool"
            | "str"
            | "error"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
    )
}

fn unsupported_slice_index_diagnostic(
    sources: &SourceMap,
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    // `std/vec` generic bodies keep parameter element facts as `Other`; user
    // call sites are preflighted before those bodies are lowered.
    if source_is_std_vec(sources, expression.span.source, nocter_home) {
        return None;
    }

    if slice_index_expression_is_buildable(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )? {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span,
        "slice indexing outside scalar, `&str`, and copy aggregate elements",
        "use `&[i32]`, `&[u8]`, `&[usize]`, `&[bool]`, `&[&str]`, or a non-empty `copy struct` element until broader slice element lowering is promoted",
    ))
}

fn slice_index_expression_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    slice_index_target_is_buildable(
        &expression.object,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn slice_index_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    match unwrap_group_expr(expression) {
        Expr::StringLiteral(_) => Some(true),
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Str => Some(true),
                TypecheckScalarViewKind::Slice(element) => {
                    typecheck_slice_element_kind_is_buildable(element).then_some(true)
                }
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_is_buildable(&return_type, resolved)
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Str => Some(true),
            TypecheckScalarViewKind::Slice(element) => {
                typecheck_slice_element_kind_is_buildable(element).then_some(true)
            }
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool => None,
        },
        Expr::Group(group) => slice_index_target_is_buildable(
            &group.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn typecheck_slice_element_kind_is_buildable(element: TypecheckSliceElementKind) -> bool {
    matches!(
        element,
        TypecheckSliceElementKind::I32
            | TypecheckSliceElementKind::U8
            | TypecheckSliceElementKind::Usize
            | TypecheckSliceElementKind::Bool
            | TypecheckSliceElementKind::Str
    )
}

fn type_expr_is_supported_slice_index_element(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_slice_element_kind(ty, resolved) != TypecheckSliceElementKind::Other
        || type_expr_is_supported_copy_aggregate_vec_element(ty, resolved)
}

fn slice_index_assignment_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypecheckSliceElementKind> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Slice(element) => Some(element),
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool
                | TypecheckScalarViewKind::Str => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_element_kind(&return_type, resolved)
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Slice(element) => Some(element),
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool
            | TypecheckScalarViewKind::Str => None,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_element_kind(
            &propagation.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_element_kind(
            &force.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_element_kind(
            &catch.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_element_kind(
            &group.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn slice_index_assignment_fallible_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypecheckSliceElementKind> {
    let Expr::Call(call) = unwrap_group_expr(expression) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    slice_index_target_type_expr_element_kind(&fallible.success, resolved)
}

fn call_return_type_expr_with_substitutions(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(specialization) =
            concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
        {
            let signature =
                resolved.method_signature_by_name_span(specialization.declaration_span)?;
            let mut return_type = signature.signature.return_type.clone();
            let self_substitution =
                HashMap::from([("Self".to_string(), specialization.self_ty.clone())]);
            return_type = substitute_type_expr_parameters(&return_type, &self_substitution);
            return_type =
                substitute_type_expr_parameters(&return_type, &specialization.substitutions);
            return Some(substitute_type_expr_parameters(
                &return_type,
                generic_substitutions,
            ));
        }

        if let Some(method_name_span) = typecheck_facts.method_call_target(member.member_span) {
            if typecheck_facts
                .generic_method_call_target(member.member_span)
                .is_some()
            {
                return None;
            }
            let method = resolved.method_signature_by_name_span(method_name_span)?;
            let mut return_type = method.signature.return_type.clone();
            if let Some(self_ty) = &method.impl_target_ty {
                let self_substitution = HashMap::from([("Self".to_string(), self_ty.clone())]);
                return_type = substitute_type_expr_parameters(&return_type, &self_substitution);
            }
            return Some(substitute_type_expr_parameters(
                &return_type,
                generic_substitutions,
            ));
        }
    }

    let signature = resolved.call_signature_for_call(call)?;
    let mut return_type = signature.return_type.clone();

    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        return_type = substitute_type_expr_parameters(&return_type, &specialization.substitutions);
    }

    Some(substitute_type_expr_parameters(
        &return_type,
        generic_substitutions,
    ))
}

fn slice_index_target_type_expr_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<bool> {
    slice_index_target_type_expr_is_buildable_inner(ty, resolved, &mut HashSet::new())
}

fn slice_index_target_type_expr_is_buildable_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<bool> {
    match ty {
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite && type_expr_resolves_to_str(&borrow.inner, resolved) =>
        {
            Some(true)
        }
        TypeExpr::Borrow(borrow) => {
            type_expr_resolves_to_supported_slice_view(&borrow.inner, resolved)
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result =
                slice_index_target_type_expr_is_buildable_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn slice_index_target_type_expr_element_kind(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<TypecheckSliceElementKind> {
    slice_index_target_type_expr_element_kind_inner(ty, resolved, &mut HashSet::new())
}

fn slice_index_target_type_expr_element_kind_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind> {
    match ty {
        TypeExpr::Borrow(borrow) => {
            if !borrow.is_readwrite && type_expr_resolves_to_str(&borrow.inner, resolved) {
                return Some(TypecheckSliceElementKind::Str);
            }
            type_expr_resolved_view_element_kind(&borrow.inner, resolved)
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result =
                slice_index_target_type_expr_element_kind_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn unsupported_std_vec_element_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    let element = std_vec_element_storage_type(
        sources,
        call,
        typecheck_facts,
        generic_substitutions,
        nocter_home,
    )?;
    if type_expr_is_supported_std_vec_element_storage(&element, resolved, call.span.source) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "`Vec` element storage outside scalar, `&str`, and copy aggregate elements",
        "use `Vec<i32>`, `Vec<u8>`, `Vec<usize>`, `Vec<bool>`, `Vec<&str>`, or a non-empty `copy struct` element until per-element drop glue is promoted",
    ))
}

fn unsupported_payload_enum_value_diagnostic(
    sources: &SourceMap,
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<Diagnostic> {
    let variant_name_span = typecheck_facts.enum_variant_target(member.member_span)?;
    let owner = resolved
        .symbols
        .symbols()
        .find_map(|symbol| match &symbol.kind {
            SymbolKind::Type(type_symbol)
                if type_symbol.kind == TypeSymbolKind::Enum
                    && type_symbol
                        .variants
                        .iter()
                        .any(|variant| variant.name_span == variant_name_span) =>
            {
                Some(type_symbol)
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })?;

    if owner
        .variants
        .iter()
        .all(|variant| variant.payload.is_empty())
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.span,
        "payload enum values",
        "use payloadless enum values, or keep payload enum construction on the `check` path until payload enum storage lowering is promoted",
    ))
}

fn std_vec_element_storage_type(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    nocter_home: Option<&Path>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref()
        && let Some(specialization) =
            concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
        && source_is_std_vec(sources, specialization.declaration_span.source, nocter_home)
        && matches!(
            declaration_name(sources, specialization.declaration_span),
            Some("push" | "reserve")
        )
    {
        return specialization.substitutions.get("T").cloned();
    }

    let specialization =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)?;
    if !source_is_std_vec(sources, specialization.declaration_span.source, nocter_home) {
        return None;
    }
    match declaration_name(sources, specialization.declaration_span)? {
        "push" | "from_slice" | "with_capacity" | "reserve" => {
            specialization.substitutions.get("T").cloned()
        }
        _ => None,
    }
}

fn type_expr_is_supported_std_vec_element_storage(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    current_source: SourceId,
) -> bool {
    if type_expr_slice_element_kind(ty, resolved) != TypecheckSliceElementKind::Other {
        return true;
    }

    if ty.span().source != current_source {
        return true;
    }

    type_expr_is_supported_copy_aggregate_vec_element(ty, resolved)
}

fn type_expr_is_supported_copy_aggregate_vec_element(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    let Ok(value) = abi_value_from_type_expr(ty, resolved) else {
        return false;
    };
    if !matches!(value.ty, AbiType::Struct(_)) || value.layout.size == 0 {
        return false;
    }
    type_expr_is_copy_struct_for_vec_element(ty, resolved, &mut HashSet::new())
}

fn type_expr_is_copy_struct_for_vec_element(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            if symbol.generic_arity > 0 {
                return false;
            }
            match symbol.kind {
                TypeSymbolKind::Struct => symbol.is_copy,
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let is_copy =
                        type_expr_is_copy_struct_for_vec_element(target, resolved, resolving_names);
                    resolving_names.remove(&symbol.canonical_name);
                    is_copy
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Generic(generic) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            if symbol.generic_arity != generic.arguments.len() {
                return false;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Struct => symbol.is_copy,
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let is_copy = type_expr_is_copy_struct_for_vec_element(
                        &target,
                        resolved,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    is_copy
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_is_copy_struct_for_vec_element(&fallible.success, resolved, resolving_names)
        }
        TypeExpr::Optional(optional) => {
            type_expr_is_copy_struct_for_vec_element(&optional.inner, resolved, resolving_names)
        }
        _ => false,
    }
}

fn type_expr_slice_element_kind(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> TypecheckSliceElementKind {
    type_expr_slice_element_kind_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_slice_element_kind_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> TypecheckSliceElementKind {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => TypecheckSliceElementKind::I32,
        TypeExpr::Reference(reference) if reference.name == "u8" => TypecheckSliceElementKind::U8,
        TypeExpr::Reference(reference) if reference.name == "usize" => {
            TypecheckSliceElementKind::Usize
        }
        TypeExpr::Reference(reference) if reference.name == "bool" => {
            TypecheckSliceElementKind::Bool
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite && type_expr_resolves_to_str(&borrow.inner, resolved) =>
        {
            TypecheckSliceElementKind::Str
        }
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return TypecheckSliceElementKind::Other;
            };
            if symbol.kind != TypeSymbolKind::Alias {
                return TypecheckSliceElementKind::Other;
            }
            let Some(target) = &symbol.alias_target else {
                return TypecheckSliceElementKind::Other;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return TypecheckSliceElementKind::Other;
            }
            let kind = type_expr_slice_element_kind_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            kind
        }
        _ => TypecheckSliceElementKind::Other,
    }
}

fn unsupported_check_only_std_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    if !matches!(
        symbol.kind,
        SymbolKind::Function(_) | SymbolKind::Primitive(_)
    ) {
        return None;
    }
    if !source_is_std_process(sources, symbol.declaration_span.source, nocter_home) {
        return None;
    }

    let declaration_name = declaration_name(sources, symbol.declaration_span)?;
    match declaration_name {
        "env" => Some(unsupported_v0_build_diagnostic(
            sources,
            call.span,
            "check-only `std/process.env` calls",
            "`std/process.env` reserves the future `&str?!` API shape; keep this code on `check` until nested fallible/optional returns and process context runtime are promoted",
        )),
        _ => None,
    }
}

fn unsupported_null_from_addr_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    if !matches!(symbol.kind, SymbolKind::Primitive(_)) {
        return None;
    }
    if !source_is_std_ptr(sources, symbol.declaration_span.source, nocter_home) {
        return None;
    }
    if declaration_name(sources, symbol.declaration_span)? != "from_addr" {
        return None;
    }
    let argument = call.arguments.first()?;
    if !expression_is_zero_integer_literal(argument) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "null raw pointer construction",
        "`*T` is non-null in v0; use `none` for `*T?` absence or pass a non-zero trusted address",
    ))
}

fn expression_is_zero_integer_literal(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value) == Some(0),
        _ => false,
    }
}

fn declaration_name(sources: &SourceMap, span: ByteSpan) -> Option<&str> {
    sources.get(span.source)?.text().get(span.start..span.end)
}

fn source_is_std_process(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
) -> bool {
    source_is_std_module(sources, source, nocter_home, Path::new("std/process.nct"))
}

fn source_is_std_ptr(sources: &SourceMap, source: SourceId, nocter_home: Option<&Path>) -> bool {
    source_is_std_module(sources, source, nocter_home, Path::new("std/ptr.nct"))
}

fn source_is_std_vec(sources: &SourceMap, source: SourceId, nocter_home: Option<&Path>) -> bool {
    source_is_std_module(sources, source, nocter_home, Path::new("std/vec.nct"))
}

fn source_is_std_module(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
    relative_path: &Path,
) -> bool {
    let Some(nocter_home) = nocter_home else {
        return false;
    };

    sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .and_then(|path| path.strip_prefix(nocter_home).ok())
        .is_some_and(|relative| relative == relative_path)
}

fn unsupported_unloaded_imported_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    let SymbolKind::Imported(imported) = &symbol.kind else {
        return None;
    };

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "unloaded imported function calls",
        &format!(
            "load `{}` from the active Nocter home or use a same-file function until imported placeholder lowering is promoted",
            imported.path
        ),
    ))
}

fn unsupported_borrow_call_argument_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let argument = call
        .arguments
        .iter()
        .enumerate()
        .find_map(|(index, argument)| {
            let parameter_ty = call_argument_parameter_type(
                call,
                index,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            if !type_expr_resolves_to_borrow(&parameter_ty, resolved) {
                return None;
            }
            match unwrap_group_expr(argument) {
                Expr::Borrow(borrow)
                    if borrow.is_readwrite
                        && !readwrite_borrow_argument_source_is_buildable(
                            &borrow.expression,
                            resolved,
                            typecheck_facts,
                            generic_substitutions,
                        ) =>
                {
                    Some(argument)
                }
                _ => None,
            }
        })?;

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "read-write borrow call arguments from unsupported expressions",
        "borrow a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary borrow lowering is promoted",
    ))
}

fn unsupported_method_borrow_receiver_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typecheck_facts.method_call_target(member.member_span)?;
    if !method_call_receiver_is_readwrite_borrow(member.member_span, typecheck_facts) {
        return None;
    }
    if readwrite_borrow_argument_source_is_buildable(
        &member.object,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.object.span(),
        "read-write method borrow receivers from unsupported expressions",
        "call the method on a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary receiver lowering is promoted",
    ))
}

fn unsupported_unspecialized_generic_method_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typecheck_facts.generic_method_call_target(member.member_span)?;
    if concrete_method_call_specialization(member, typecheck_facts, generic_substitutions).is_some()
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "generic impl method calls without concrete type arguments",
        "call the method through a receiver whose generic arguments are concrete until generic method bodies can be re-specialized recursively",
    ))
}

fn concrete_method_call_specialization(
    member: &crate::ast::MemberExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<MethodCallSpecialization> {
    typecheck_facts
        .method_call_specialization(member.member_span)?
        .with_context_substitutions(generic_substitutions)
}

fn unsupported_unspecialized_generic_function_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    typecheck_facts.generic_function_call_target(call.span)?;
    if concrete_function_call_specialization(call, typecheck_facts, generic_substitutions).is_some()
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "generic function calls without concrete type arguments",
        "make every generic parameter concrete through argument types or return context",
    ))
}

fn concrete_function_call_specialization(
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<FunctionCallSpecialization> {
    typecheck_facts
        .function_call_specialization(call.span)?
        .with_context_substitutions(generic_substitutions)
}

fn method_call_receiver_is_readwrite_borrow(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    let Some((_span, label)) = typecheck_facts.call_hover_at_offset(member_span.start) else {
        return false;
    };
    label
        .strip_prefix("method ")
        .is_some_and(|label| label.starts_with("&+"))
}

fn readwrite_borrow_argument_source_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
        Expr::Index(index) => slice_index_assignment_element_kind(
            &index.object,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
        .is_some_and(typecheck_slice_element_kind_is_buildable),
        _ => false,
    }
}

fn aggregate_member_root_is_identifier(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
        _ => false,
    }
}

fn type_expr_resolves_to_borrow(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_resolves_to_borrow_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_resolves_to_borrow_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Borrow(_) => true,
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let resolves = type_expr_resolves_to_borrow_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            resolves
        }
        _ => false,
    }
}

fn unwrap_group_expr(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group_expr(&group.expression),
        _ => expression,
    }
}

fn call_target_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
) -> Option<CallTarget> {
    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        return Some(call_target_for_source(
            specialization.declaration_span.source,
            root_source,
            specialization.target_name.clone(),
        ));
    }

    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(method_name_span) = typecheck_facts.method_call_target(member.member_span) {
            let target_name = if typecheck_facts
                .generic_method_call_target(member.member_span)
                .is_some()
            {
                concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)?
                    .target_name
            } else {
                names.get(&method_name_span).cloned()?
            };
            return Some(call_target_for_source(
                method_name_span.source,
                root_source,
                target_name,
            ));
        }
        if let Some((_owner, function)) = resolved.associated_function_for_call(call) {
            return Some(call_target_for_source(
                function.name_span.source,
                root_source,
                function.target_name.clone(),
            ));
        }
    }

    let Expr::Identifier(_) = call.callee.as_ref() else {
        return None;
    };
    let symbol = resolved.symbol_for_call(call)?;
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
            let target_name = if symbol.declaration_span.source != root_source {
                names
                    .get(&symbol.declaration_span)
                    .cloned()
                    .unwrap_or_else(|| symbol.name.clone())
            } else {
                symbol.name.clone()
            };
            Some(call_target_for_source(
                symbol.declaration_span.source,
                root_source,
                target_name,
            ))
        }
        SymbolKind::Imported(_) => None,
    }
}

fn unsupported_v0_build_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    construct: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0435",
        format!("Nocter v0 build cannot lower {construct} yet"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}

fn call_target_for_source(source: SourceId, root_source: SourceId, name: String) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}

fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", type_expr_display_lossy(self_ty))
}

fn nested_fallible_return_issue(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> Option<BuildabilityIssue> {
    nested_fallible_return_type_issue(&function.return_type, resolved)
}

fn nested_fallible_return_type_issue(
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<BuildabilityIssue> {
    if type_expr_fallible_depth(return_type, resolved) <= 1 {
        return None;
    }

    Some(BuildabilityIssue {
        span: return_type.span(),
        construct: "nested fallible or optional return types",
        help: "flatten the return boundary to a single optional or fallible layer until nested fallible lowering is promoted",
    })
}

fn type_expr_fallible_depth(ty: &TypeExpr, resolved: &ResolveOutput) -> usize {
    type_expr_fallible_depth_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_fallible_depth_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> usize {
    match ty {
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return 0;
            };
            let Some(target) = &symbol.alias_target else {
                return 0;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return 0;
            }
            let depth = type_expr_fallible_depth_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            depth
        }
        TypeExpr::Fallible(fallible) => {
            1 + type_expr_fallible_depth_inner(&fallible.success, resolved, resolving_names)
        }
        TypeExpr::Optional(optional) => {
            1 + type_expr_fallible_depth_inner(&optional.inner, resolved, resolving_names)
        }
        _ => 0,
    }
}

fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CompileUnit, analyze_executable_compile_unit};
    use crate::lexer::lex;
    use crate::parser::parse;
    use std::collections::HashMap;

    #[test]
    fn reports_reachable_unloaded_imported_call_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"use std/io.print

func main(): i32 {
    print("hello")
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower unloaded imported function calls yet"
        );
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some(
                "load `std/io` from the active Nocter home or use a same-file function until imported placeholder lowering is promoted"
            )
        );
        assert!(diagnostics[0].primary_span.is_some());
    }

    #[test]
    fn does_not_report_unreachable_unloaded_imported_call() {
        let (sources, analysis) = analyze_text(
            r#"use std/io.print

func main(): i32 {
    return 0
}

func unused(): i32 {
    print("hello")
    return 1
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_str_equality() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    if "a" == "b" {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_unreachable_str_equality() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return 0
}

func unused(): bool {
    return "a" == "b"
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payloadless_enum_equality() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    if Choice.yes == Choice.no {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payloadless_if_is() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    if choice is Choice.yes {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payloadless_match() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes {
            return 0
        }

        else {
            return 1
        }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_terminal_if_expression_body_result() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    let ok = true
    if ok {
        0
    } else {
        1
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_terminal_match_expression_body_result() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes { 0 }
        else { 1 }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_terminal_match_expression_return_statement() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    return match choice {
        Choice.yes { 0 }
        else { 1 }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_reachable_payload_match_expression_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return match result {
        Result.ok(value) { value }
        else { 0 }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("`match` expressions")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn reports_reachable_payload_enum_construction_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("payload enum values"));
    }

    #[test]
    fn reports_reachable_payload_enum_member_value_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.failed
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("payload enum values"));
    }

    #[test]
    fn reports_reachable_scope_drop_body_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

func main(): i32 {
    let resource = Resource{ value: 1 }
    return resource.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("array literals"));
    }

    #[test]
    fn reports_reachable_generic_scope_drop_body_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

impl<T> Box<T> {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 1 }
    return box.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("array literals"));
    }

    #[test]
    fn reports_reachable_field_replacement_drop_body_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

struct Holder {
    inner: Resource
}

func main(): i32 {
    var holder = Holder{ inner: Resource{ value: 1 } }
    holder.inner = Resource{ value: 2 }
    return holder.inner.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("array literals"));
    }

    #[test]
    fn reports_reachable_generic_field_replacement_drop_body_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let bytes: [u8; 2] = [1, 2]
        return
    }
}

struct Holder<T> {
    inner: T
}

func main(): i32 {
    var holder = Holder<Resource>{ inner: Resource{ value: 1 } }
    holder.inner = Resource{ value: 2 }
    return holder.inner.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("array literals"));
    }

    #[test]
    fn does_not_report_unreachable_payloadless_enum_equality() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    return 0
}

func unused(): bool {
    return Choice.yes == Choice.no
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_member_rooted_slice_index_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"struct Buffer {
    pub bytes: &+[u8]
}

func main(): i32 {
    let holder = Buffer{ bytes: buffer() }
    holder.bytes[0] = 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_direct_slice_binding_index_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    let bytes = buffer()
    bytes[0] = 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_concrete_generic_struct_literal() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32>{
        value: 42,
    }
    return box.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_concrete_generic_instantiation_signature() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return make().value
}

func make(): Box<i32> {
    return Box<i32>{
        value: 42,
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_function_with_concrete_arguments() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return identity(42)
}

func identity<T>(value: T): T {
    return value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_nested_generic_function_with_concrete_arguments() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return forward(42)
}

func forward<T>(value: T): T {
    return identity(value)
}

func identity<T>(value: T): T {
    return value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_function_body_method_call_with_concrete_arguments() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return forward(move box)
}

func forward<T>(box: Box<T>): T {
    return (move box).into_value()
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_function_with_expected_return_type() {
        let (sources, analysis) = analyze_text(
            r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    let marker: Marker<u8> = make()
    return marker.code
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_function_in_catch_return_with_expected_return_type() {
        let (sources, analysis) = analyze_text(
            r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    return recover().code
}

func recover(): Marker<u8> {
    return source() catch error {
        return make()
    }
}

func source(): Marker<u8>! {
    return Marker<u8>{ code: 1 }
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_function_with_parameter_expected_type() {
        let (sources, analysis) = analyze_text(
            r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    return consume(make())
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_nested_generic_function_with_parameter_expected_type() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Marker<T> {
    code: i32
}

func main(): i32 {
    return consume(forward(make()))
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func forward<T>(value: T): T {
    return value
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_unspecialized_generic_function_call_inside_reachable_specialization() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return forward(42)
}

func forward<T>(value: T): T {
    let optional = empty()
    return value
}

func empty<T>(): T? {
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower generic function calls without concrete type arguments yet"
        );
    }

    #[test]
    fn reports_reachable_unspecialized_generic_function_call() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    let value = empty()
    return 0
}

func empty<T>(): T? {
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower generic function calls without concrete type arguments yet"
        );
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some("make every generic parameter concrete through argument types or return context")
        );
    }

    #[test]
    fn accepts_reachable_concrete_generic_impl_method() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method &self.read(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return box.read()
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_impl_method_with_concrete_receiver() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32>{ value: 42 }
    return (move box).into_value()
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_unreachable_generic_struct_literal() {
        let (sources, analysis) = analyze_text(
            r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return 0
}

func unused(): i32 {
    let box = Box<i32>{
        value: 42,
    }
    return box.value
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    fn analyze_text(text: &str) -> (SourceMap, crate::analysis::CompileUnitAnalysis) {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let lexed = lex(&sources, source);
        assert!(
            lexed.diagnostics.is_empty(),
            "unexpected lex diagnostics: {:?}",
            lexed.diagnostics
        );
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(
            parsed.diagnostics.is_empty(),
            "unexpected parse diagnostics: {:?}",
            parsed.diagnostics
        );
        let ast = parsed.ast.expect("expected ast");
        let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), HashMap::new(), None);
        let analysis = analyze_executable_compile_unit(&sources, &unit);
        let diagnostics = analysis.diagnostics();
        assert!(
            diagnostics.is_empty(),
            "unexpected frontend diagnostics: {diagnostics:?}"
        );

        (sources, analysis)
    }
}
