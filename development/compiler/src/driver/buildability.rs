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
    MethodDecl, OtherwiseExpr, Parameter, Stmt, StructLiteralField, SwitchPayloadPattern, TypeExpr,
    UnaryOperator, substitute_type_expr_parameters, type_expr_display_lossy,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::CallTarget;
use crate::literals::decode_integer_literal_value;
use crate::resolve::{ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{
    FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts,
    TypecheckMethodReceiverKind, TypecheckScalarViewKind, TypecheckSliceElementKind,
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
        issues.extend(nested_fallible_return_issue(
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
        issues.extend(nested_fallible_return_issue(
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
        issues.extend(nested_fallible_return_type_issue(
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

fn callable_function_signature_issues(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    root_source: SourceId,
) -> Vec<BuildabilityIssue> {
    let mut issues = callable_parameter_issues(
        &function.parameters.parameters,
        substitutions,
        resolved,
        resolved_sources,
    );
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(&return_type, resolved, &source_resolver)
        && !function_error_return_type_is_buildable(
            function,
            &return_type,
            resolved,
            &source_resolver,
            root_source,
        )
    {
        issues.push(BuildabilityIssue {
            span: function.return_type.span(),
            construct: "function return types outside the v0 runtime ABI subset",
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, a supported aggregate, a supported static `error` payload helper, or a fallible form with a non-`error` success type",
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
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, a supported aggregate, or a fallible form with a non-`error` success type",
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
                help: "use `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `error`, scalar borrow parameters, aggregate borrow parameters, or supported aggregate value parameters",
            })
        })
        .collect()
}

fn function_error_return_type_is_buildable<'a, F>(
    function: &FunctionDecl,
    return_type: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
    root_source: SourceId,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !type_expr_is_error_parameter_with_resolver(return_type, resolved, resolver) {
        return false;
    }

    non_root_error_constructor_signature(function, root_source, resolved, resolver)
        || (function.parameters.parameters.is_empty()
            && static_error_payload_body_is_buildable(
                &function.body,
                root_source,
                resolved,
                resolver,
            ))
}

fn non_root_error_constructor_signature<'a, F>(
    function: &FunctionDecl,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    function.name_span.source != root_source
        && error_constructor_signature_is_buildable_with_resolver(
            function
                .parameters
                .parameters
                .iter()
                .map(|parameter| &parameter.ty),
            &function.return_type,
            resolved,
            resolver,
        )
}

fn static_error_payload_body_is_buildable<'a, F>(
    body: &Block,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let mut runtime_statements = body
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)));
    let Some(Stmt::Return(statement)) = runtime_statements.next() else {
        return false;
    };
    if runtime_statements.next().is_some() {
        return false;
    }
    let Some(expression) = statement.expression.as_ref() else {
        return false;
    };
    static_error_payload_expression_is_buildable(expression, root_source, resolved, resolver)
}

fn static_error_payload_expression_is_buildable<'a, F>(
    expression: &Expr,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match expression {
        Expr::Group(group) => static_error_payload_expression_is_buildable(
            &group.expression,
            root_source,
            resolved,
            resolver,
        ),
        Expr::Call(call) => {
            error_constructor_call_is_buildable(call, root_source, resolved, resolver)
                && call.arguments.len() == 2
                && call
                    .arguments
                    .iter()
                    .all(static_error_payload_string_expression_is_buildable)
        }
        _ => false,
    }
}

fn static_error_payload_string_expression_is_buildable(expression: &Expr) -> bool {
    match expression {
        Expr::StringLiteral(_) => true,
        Expr::Group(group) => {
            static_error_payload_string_expression_is_buildable(&group.expression)
        }
        _ => false,
    }
}

fn error_constructor_call_is_buildable<'a, F>(
    call: &CallExpr,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let Some(symbol) = resolved.symbol_for_call(call)
        && symbol.declaration_span.source != root_source
        && let SymbolKind::Function(signature) | SymbolKind::Primitive(signature) = &symbol.kind
    {
        return error_constructor_signature_is_buildable_with_resolver(
            signature.parameters.iter().map(|parameter| &parameter.ty),
            &signature.return_type,
            resolved,
            resolver,
        );
    }

    if let Some((_owner, function)) = resolved.associated_function_for_call(call)
        && function.name_span.source != root_source
    {
        return error_constructor_signature_is_buildable_with_resolver(
            function
                .signature
                .parameters
                .iter()
                .map(|parameter| &parameter.ty),
            &function.signature.return_type,
            resolved,
            resolver,
        );
    }

    false
}

fn error_constructor_signature_is_buildable_with_resolver<'a, 't, F, I>(
    parameter_types: I,
    return_type: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
    I: IntoIterator<Item = &'t TypeExpr>,
{
    let mut parameter_types = parameter_types.into_iter();
    let Some(code_type) = parameter_types.next() else {
        return false;
    };
    let Some(message_type) = parameter_types.next() else {
        return false;
    };
    if parameter_types.next().is_some() {
        return false;
    }

    type_expr_has_str_view_abi_with_resolver(code_type, fallback_resolved, resolver)
        && type_expr_has_str_view_abi_with_resolver(message_type, fallback_resolved, resolver)
        && type_expr_is_error_parameter_with_resolver(return_type, fallback_resolved, resolver)
}

fn type_expr_has_str_view_abi_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver)
        .is_ok_and(|value| matches!(value.ty, AbiType::StrView))
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
    resolved_sources: &ResolvedSources<'_>,
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
        callable.return_type.as_ref(),
        sources,
        callable.resolved,
        callable.typecheck_facts,
        &callable.substitutions,
        root_source,
        names,
        resolved_sources,
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
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = result {
        collect_terminal_return_expression_diagnostics(
            result,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = result {
        collect_expression_diagnostics(
            result,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn reachable_block_parts_for_buildability<'a>(
    statements: &'a [Stmt],
    result: Option<&'a Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> (&'a [Stmt], Option<&'a Expr>) {
    for (index, statement) in statements.iter().enumerate() {
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ) {
            return (&statements[..=index], None);
        }
    }

    (statements, result)
}

fn collect_terminal_return_expression_diagnostics(
    expression: &Expr,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(expression) => {
            collect_otherwise_return_expression_diagnostics(
                expression,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::ArrayLiteral(_)
            if fixed_array_literal_return_has_fixed_array_type(
                expression,
                return_type,
                resolved,
                resolved_sources,
            ) =>
        {
            collect_fixed_array_literal_elements_diagnostics(
                unwrap_group_expr(expression),
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::If(expression) if terminal_if_expression_is_buildable(expression) => {
            collect_terminal_control_condition_move_diagnostics(
                &expression.condition,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::IfIs(expression)
            if terminal_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Match(expression)
            if terminal_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_terminal_return_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_terminal_return_block_diagnostics(
                    &wildcard_arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
    }
}

fn collect_value_expression_diagnostics(
    expression: &Expr,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression) if value_if_expression_is_buildable(expression) => {
            collect_control_condition_move_diagnostics(
                &expression.condition,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::IfIs(expression)
            if value_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Match(expression)
            if value_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_value_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_value_block_diagnostics(
                    &wildcard_arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Otherwise(expression) => {
            collect_otherwise_scalar_view_value_expression_diagnostics(
                expression,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        _ => collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
    }
}

fn collect_value_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    let Some(result) = &block.result else {
        return;
    };
    collect_value_expression_diagnostics(
        result,
        return_type,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_return_expression_diagnostics(
    expression: &OtherwiseExpr,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 return subset",
            "end runtime-shipped `otherwise` return fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_otherwise_return_fallback_block_diagnostics(
        &expression.fallback,
        return_type,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_binding_initializer_diagnostics(
    expression: &OtherwiseExpr,
    binding_is_scalar_or_view: bool,
    binding_fixed_array_type: Option<&TypeExpr>,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_binding_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 binding subset",
            "end runtime-shipped `otherwise` binding fallbacks with a value, direct `return`, loop-local `break`/`continue`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        binding_fixed_array_type,
        binding_is_scalar_or_view,
        return_type,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_assignment_value_diagnostics(
    expression: &OtherwiseExpr,
    assignment_aggregate_type: Option<&TypeExpr>,
    assignment_is_scalar_or_view: bool,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 assignment subset",
            "end runtime-shipped `otherwise` assignment fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        assignment_aggregate_type,
        assignment_is_scalar_or_view,
        return_type,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_scalar_view_value_expression_diagnostics(
    expression: &OtherwiseExpr,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 scalar/view value subset",
            "end runtime-shipped scalar/view `otherwise` value fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        None,
        true,
        return_type,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_aggregate_value_expression_diagnostics(
    expression: &OtherwiseExpr,
    expected_type: &TypeExpr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 aggregate value subset",
            "end runtime-shipped aggregate `otherwise` fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        Some(expected_type),
        false,
        None,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_otherwise_runtime_value_diagnostics(
    expression: &OtherwiseExpr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if otherwise_optional_value_call_is_buildable(
        &expression.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) {
        return;
    }

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        expression.value.span(),
        "`otherwise` values outside the v0 runtime subset",
        "apply runtime-shipped `otherwise` directly to a call returning a top-level optional value",
    ));
}

fn collect_otherwise_return_fallback_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if block.result.is_none() {
        collect_block_diagnostics(
            block,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
        return;
    }

    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = &block.result {
        collect_terminal_return_expression_diagnostics(
            result,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn collect_otherwise_value_fallback_block_diagnostics(
    block: &Block,
    expected_aggregate_type: Option<&TypeExpr>,
    result_is_scalar_or_view: bool,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if block.result.is_none() {
        collect_block_diagnostics(
            block,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
        return;
    }

    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = &block.result {
        if fixed_array_literal_for_type_has_fixed_array_type(
            result,
            expected_aggregate_type,
            resolved,
            resolved_sources,
        ) {
            collect_fixed_array_literal_elements_diagnostics(
                unwrap_group_expr(result),
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        } else if result_is_scalar_or_view {
            collect_value_expression_diagnostics(
                result,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        } else {
            collect_expression_diagnostics(
                result,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
    }
}

fn collect_void_effect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_control_condition_move_diagnostics(
        &expression.condition,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
    collect_expression_diagnostics(
        &expression.condition,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
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
        resolved_sources,
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
            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
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
        resolved_sources,
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
        resolved_sources,
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
            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
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
        resolved_sources,
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
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(wildcard_arm) = &expression.wildcard_arm {
        collect_void_effect_block_diagnostics(
            &wildcard_arm.body,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            None,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
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
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn binding_initializer_may_use_value_control_expression(
    statement: &crate::ast::BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let ty = statement.ty.clone().or_else(|| {
        typecheck_facts
            .binding_type_expr(statement.name_span)
            .cloned()
    });
    let Some(ty) = ty else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
}

fn assignment_value_may_use_value_control_expression(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            typecheck_facts
                .binding_type_expr(symbol.name_span)
                .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
                .is_some_and(|ty| {
                    type_expr_is_buildable_scalar_or_view_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
                })
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
    resolved_sources: &ResolvedSources<'_>,
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
    .is_some_and(|ty| {
        type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
    })
}

fn otherwise_aggregate_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(argument) else {
        return None;
    };
    let ty = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

fn otherwise_aggregate_struct_field_type(
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(&field.value) else {
        return None;
    };
    let ty = field_type_expr_for_span(field.name_span, resolved, typecheck_facts)?;
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

fn otherwise_aggregate_member_root_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let otherwise = aggregate_member_root_otherwise(&member.object)?;
    let Expr::Call(call) = unwrap_group_expr(&otherwise.value) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let ty = type_expr_top_level_optional_success_with_resolver(
        &return_type,
        resolved,
        &source_resolver,
    )?;
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

fn aggregate_member_root_otherwise(expression: &Expr) -> Option<&OtherwiseExpr> {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(otherwise) => Some(otherwise),
        Expr::Member(member) => aggregate_member_root_otherwise(&member.object),
        _ => None,
    }
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

fn fixed_array_literal_argument_has_fixed_array_parameter_type(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(argument) else {
        return false;
    };
    let Some(ty) = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources).is_some()
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

fn fixed_array_literal_struct_field_has_fixed_array_type(
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(&field.value) else {
        return false;
    };
    let Some(ty) = field_type_expr_for_span(field.name_span, resolved, typecheck_facts) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources).is_some()
}

fn unsupported_local_binding_type_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if fixed_array_literal_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_copy_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_call_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_member_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    let fixed_array_binding_type = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    );
    if fixed_array_binding_type.is_some() {
        return Some(match unwrap_group_expr(&statement.initializer) {
            Expr::ArrayLiteral(_) => unsupported_v0_build_diagnostic(
                sources,
                statement.initializer.span(),
                "fixed array local bindings outside supported literal values",
                "match the fixed array length and use `i32`, `u8`, `usize`, `bool`, or `&str` elements until broader fixed array element storage is promoted",
            ),
            _ => unsupported_v0_build_diagnostic(
                sources,
                statement.name_span,
                "fixed array local bindings outside supported initialization",
                "initialize fixed array locals directly from a supported array literal, copy another supported fixed array local or aggregate field, or bind a matching fixed array call result until broader fixed array move lowering is promoted",
            ),
        });
    }

    if local_binding_type_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        statement.name_span,
        "local bindings with unsupported value types",
        "bind `i32`, `u8`, `usize`, `bool`, `&str`, slice views, payloadless enums, errors, aggregate values, or supported fixed array literals until broader scalar local lowering is promoted",
    ))
}

fn local_binding_type_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if let Some(ty) = &statement.ty {
        let ty = substitute_type_expr_parameters(ty, generic_substitutions);
        return local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
            || !type_expr_is_known_unsupported_scalar_value_for_sources(
                &ty,
                resolved,
                resolved_sources,
            );
    }

    if typecheck_facts
        .binding_scalar_view_kind(statement.name_span)
        .is_some()
    {
        return true;
    }

    typecheck_facts
        .binding_type_expr(statement.name_span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_none_or(|ty| {
            local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
                || !type_expr_is_known_unsupported_scalar_value_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
        })
}

fn unsupported_scalar_type_label(label: &str) -> bool {
    matches!(
        label,
        "i8" | "i16" | "i64" | "isize" | "u16" | "u32" | "u64"
    )
}

fn local_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_error_parameter_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
}

fn fixed_array_literal_return_has_fixed_array_type(
    expression: &Expr,
    return_type: Option<&TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(expression) else {
        return false;
    };
    return_type
        .and_then(|ty| fixed_array_return_type_abi(ty, resolved, resolved_sources))
        .is_some()
}

fn fixed_array_literal_for_type_has_fixed_array_type(
    expression: &Expr,
    ty: Option<&TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(expression) else {
        return false;
    };
    ty.and_then(|ty| fixed_array_type_abi_for_sources(ty, resolved, resolved_sources))
        .is_some()
}

fn fixed_array_literal_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some(ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    else {
        return false;
    };
    let Some((element, length, _layout)) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
    else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && fixed_array_element_abi_is_buildable(&element)
}

fn fixed_array_copy_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Identifier(identifier) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = local_identifier_type_expr_with_substitutions(
        identifier,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_call_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some((target_element, target_length, target_layout)) = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = fixed_array_binding_call_result_type_expr(
        &statement.initializer,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_member_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Member(member) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };

    let Some(source_ty) = field_type_expr_for_member(member, resolved, typecheck_facts) else {
        return false;
    };
    let source_ty = substitute_type_expr_parameters(&source_ty, generic_substitutions);
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_copy_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = local_identifier_type_expr_with_substitutions(
        identifier,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_call_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = fixed_array_call_result_type_expr(
        &statement.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_otherwise_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    if !matches!(unwrap_group_expr(&statement.value), Expr::Otherwise(_)) {
        return false;
    }
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = fixed_array_binding_call_result_type_expr(
        &statement.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_member_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Member(member) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };

    let Some(source_ty) = field_type_expr_for_member(member, resolved, typecheck_facts) else {
        return false;
    };
    let source_ty = substitute_type_expr_parameters(&source_ty, generic_substitutions);
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

fn fixed_array_abi_matches_buildable_element(
    target_element: &AbiType,
    target_length: u64,
    target_layout: crate::abi::ValueLayout,
    source_element: &AbiType,
    source_length: u64,
    source_layout: crate::abi::ValueLayout,
) -> bool {
    target_element == source_element
        && target_length == source_length
        && target_layout == source_layout
        && fixed_array_element_abi_is_buildable(source_element)
}

fn fixed_array_binding_call_result_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(otherwise) => {
            let Expr::Call(call) = unwrap_group_expr(&otherwise.value) else {
                return None;
            };
            fixed_array_inner_type_expr_from_optional_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => fixed_array_call_result_type_expr(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
    }
}

fn fixed_array_call_result_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => call_return_type_expr_with_substitutions(
            call,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group_expr(&propagation.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group_expr(&force.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group_expr(&catch.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => None,
    }
}

fn fixed_array_success_type_expr_from_fallible_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    Some(*fallible.success)
}

fn fixed_array_inner_type_expr_from_optional_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Optional(optional) = return_type else {
        return None;
    };
    Some(*optional.inner)
}

fn fixed_array_literal_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((element, length, _layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && fixed_array_element_abi_is_buildable(&element)
}

fn unsupported_fixed_array_assignment_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )?;

    if fixed_array_literal_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_copy_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_call_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_otherwise_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_member_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(match unwrap_group_expr(&statement.value) {
        Expr::ArrayLiteral(_) => unsupported_v0_build_diagnostic(
            sources,
            statement.value.span(),
            "fixed array assignments outside supported literal values",
            "match the target fixed array length and use `i32`, `u8`, `usize`, `bool`, or `&str` elements until broader fixed array element storage is promoted",
        ),
        _ => unsupported_v0_build_diagnostic(
            sources,
            statement.target.span(),
            "fixed array assignments outside supported replacement values",
            "assign a matching fixed array literal, copy another matching local or aggregate-field fixed array, or assign a matching fixed array call result until broader fixed array expression lowering is promoted",
        ),
    })
}

fn fixed_array_assignment_target_abi(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let ty = fixed_array_assignment_target_type_expr(
        target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
}

fn fixed_array_assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let ty = assignment_target_type_expr(target, resolved, typecheck_facts, generic_substitutions)?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)?;
    Some(ty)
}

fn aggregate_assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let ty = assignment_target_type_expr(target, resolved, typecheck_facts, generic_substitutions)?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

fn assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(target) {
        Expr::Identifier(identifier) => Some(local_identifier_type_expr_with_substitutions(
            identifier,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )?),
        Expr::Member(member) => {
            let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
            Some(substitute_type_expr_parameters(&ty, generic_substitutions))
        }
        _ => None,
    }
}

fn fixed_array_binding_type_abi(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let ty =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
}

fn fixed_array_return_type_abi(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    match ty {
        TypeExpr::Fallible(fallible) => {
            fixed_array_return_type_abi(&fallible.success, resolved, resolved_sources)
        }
        TypeExpr::Optional(optional) => {
            fixed_array_return_type_abi(&optional.inner, resolved, resolved_sources)
        }
        _ => fixed_array_type_abi_for_sources(ty, resolved, resolved_sources),
    }
}

fn binding_type_expr_with_substitutions(
    statement: &BindingStmt,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    statement
        .ty
        .clone()
        .or_else(|| {
            typecheck_facts
                .binding_type_expr(statement.name_span)
                .cloned()
        })
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}

fn local_identifier_type_expr_with_substitutions(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let symbol = resolved.local_symbol_for_identifier(identifier)?;
    typecheck_facts
        .binding_type_expr(symbol.name_span)
        .cloned()
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}

fn fixed_array_type_abi_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let value =
        abi_value_from_type_expr_with_resolver(ty, fallback_resolved, source_resolver).ok()?;
    let layout = value.layout;
    match value.ty {
        AbiType::Array { element, length } => Some((*element, length, layout)),
        _ => None,
    }
}

fn fixed_array_element_abi_is_buildable(element: &AbiType) -> bool {
    matches!(
        element,
        AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::StrView
    )
}

fn type_expr_is_known_unsupported_scalar_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_known_unsupported_scalar_value_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}

fn type_expr_is_known_unsupported_scalar_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_known_unsupported_scalar_value_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn type_expr_is_known_unsupported_scalar_value_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if unsupported_scalar_type_label(&reference.name) => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok_and(
            |value| {
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
            },
        ),
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
    let source_resolved = resolver(ty.span().source);
    let Some(name) = type_expr_symbol_name(ty) else {
        return source_resolved.unwrap_or(fallback_resolved);
    };

    if let Some(resolved) = source_resolved
        && type_symbol_by_reference_name(resolved, name).is_some()
    {
        return resolved;
    }
    if type_symbol_by_reference_name(fallback_resolved, name).is_some() {
        return fallback_resolved;
    }

    source_resolved.unwrap_or(fallback_resolved)
}

fn type_expr_symbol_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn type_symbol_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        short_qualified_type_name(name)
            .and_then(|short| resolved.type_symbol_by_reference_name(short))
    })
}

fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}

fn type_expr_is_top_level_optional_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_top_level_optional_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_top_level_optional_success_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_top_level_optional_success_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_top_level_optional_success_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(optional) => Some(optional.inner.as_ref().clone()),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_top_level_optional_success_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn type_expr_is_top_level_optional_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(_) => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_top_level_optional_inner(
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

fn type_expr_is_buildable_scalar_or_view_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, &source_resolver)
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok_and(|value| {
        matches!(
            value.ty,
            AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::Pointer
        )
    })
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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

fn type_expr_resolves_to_supported_slice_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_supported_slice_view_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn type_expr_resolves_to_supported_slice_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(view) => Some(type_expr_is_supported_slice_index_element_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        )),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_resolves_to_supported_slice_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn type_expr_resolved_view_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolved_view_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_resolved_view_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(view) => Some(type_expr_slice_element_kind_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        )),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_resolved_view_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
        || type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
}

fn type_expr_is_error_parameter_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_error_parameter_with_resolver(ty, fallback_resolved, &source_resolver)
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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

fn type_expr_is_supported_aggregate_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, &source_resolver)
}

fn type_expr_is_supported_aggregate_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver) else {
        return false;
    };
    match &value.ty {
        AbiType::Enum(_) => {
            type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, resolver)
        }
        _ => abi_value_is_supported_aggregate_value(&value),
    }
}

fn abi_value_is_supported_aggregate_value(value: &AbiValue) -> bool {
    match &value.ty {
        AbiType::Struct(_) => value.layout.size > 0 && !abi_type_contains_enum(&value.ty),
        AbiType::Array { element, .. } => {
            fixed_array_element_abi_is_buildable(element) && !abi_type_contains_enum(element)
        }
        _ => false,
    }
}

fn abi_type_contains_enum(ty: &AbiType) -> bool {
    match ty {
        AbiType::Enum(_) => true,
        AbiType::Array { element, .. } => abi_type_contains_enum(element),
        AbiType::Struct(fields) => fields.iter().any(|field| abi_type_contains_enum(&field.ty)),
        AbiType::Bool
        | AbiType::U8
        | AbiType::I8
        | AbiType::U16
        | AbiType::I16
        | AbiType::U32
        | AbiType::I32
        | AbiType::U64
        | AbiType::I64
        | AbiType::Usize
        | AbiType::Isize
        | AbiType::Pointer
        | AbiType::Borrow
        | AbiType::StrView
        | AbiType::SliceView => false,
    }
}

fn type_expr_is_supported_payload_enum_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, &source_resolver)
}

fn type_expr_is_supported_payload_enum_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_supported_payload_enum_value_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn type_expr_is_supported_payload_enum_value_inner<'a, F>(
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            match symbol.kind {
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let result = type_expr_is_supported_payload_enum_value_inner(
                        target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Enum if symbol.generic_arity == 0 => {
                    type_symbol_payload_enum_payloads_are_supported_values(
                        symbol,
                        fallback_resolved,
                        resolver,
                        &HashMap::new(),
                    )
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
                return false;
            };
            if symbol.generic_arity != generic.arguments.len() {
                return false;
            }
            let substitutions: HashMap<String, TypeExpr> = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let result = type_expr_is_supported_payload_enum_value_inner(
                        &target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Enum => type_symbol_payload_enum_payloads_are_supported_values(
                    symbol,
                    fallback_resolved,
                    resolver,
                    &substitutions,
                ),
                TypeSymbolKind::Struct | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => false,
    }
}

fn type_symbol_payload_enum_payloads_are_supported_values<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if symbol.kind != TypeSymbolKind::Enum
        || symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    symbol.variants.iter().all(|variant| {
        payload_enum_variant_payloads_are_supported(
            &variant.payload,
            fallback_resolved,
            resolver,
            substitutions,
        )
    })
}

fn payload_enum_variant_payloads_are_supported<'a, F>(
    payloads: &[crate::resolve::ParameterSignature],
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match payloads {
        [] => true,
        [payload] => {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }
        payloads => payloads.iter().all(|payload| {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }),
    }
}

fn payload_enum_payload_type_is_supported<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    allow_active_drop: bool,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok()
        && (type_expr_is_runtime_copy_value_with_resolver(
            ty,
            fallback_resolved,
            resolver,
            &mut HashSet::new(),
        ) || (allow_active_drop
            && type_expr_has_direct_drop_with_resolver(
                ty,
                fallback_resolved,
                resolver,
                &mut HashSet::new(),
            )))
}

fn type_expr_has_direct_drop_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
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
            (generic.name.as_str(), substitutions)
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => return false,
    };

    let Some(symbol) = type_symbol_by_reference_name(resolved, type_name) else {
        return false;
    };
    if symbol.kind == TypeSymbolKind::Alias {
        let Some(target) = symbol.alias_target.as_ref() else {
            return false;
        };
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return false;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let has_drop = type_expr_has_direct_drop_with_resolver(
            &target,
            fallback_resolved,
            resolver,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return has_drop;
    }

    symbol.drop_member.is_some()
}

fn value_if_expression_is_buildable(expression: &crate::ast::IfStmt) -> bool {
    expression.else_block.is_some()
        && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

fn value_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_if_is_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

fn value_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_match_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression
        .arms
        .iter()
        .all(|arm| value_block_is_buildable(&arm.body))
        && expression
            .wildcard_arm
            .as_ref()
            .is_none_or(|arm| value_block_is_buildable(&arm.body))
}

fn value_block_is_buildable(block: &Block) -> bool {
    block.result.is_some()
        && block
            .statements
            .iter()
            .all(value_block_leading_statement_is_buildable)
}

fn value_block_leading_statement_is_buildable(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Expression(_)
    )
}

fn void_effect_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn void_effect_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if_is_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn void_effect_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_switch_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.arms.iter().all(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && expression.wildcard_arm.as_ref().is_none_or(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn void_effect_block_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match block.result.as_deref() {
        Some(result) => void_effect_expression_is_buildable(
            result,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        None => true,
    }
}

fn void_effect_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(expression) => void_effect_if_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(expression) => void_effect_if_is_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(expression) => void_effect_match_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => expression_statement_is_supported(
            expression,
            resolved,
            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    expression.else_block.is_some()
        && if_is_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

fn terminal_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let expression_is_exhaustive = expression.wildcard_arm.is_some()
        || switch_statement_covers_all_payloadless_variants(expression, resolved)
        || switch_statement_covers_all_tag_only_payload_variants(expression, resolved);

    expression_is_exhaustive
        && (payloadless_switch_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ) || tag_only_payload_enum_switch_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ))
}

fn payloadless_if_is_statement_is_buildable(
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

fn if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_if_is_statement_is_buildable(statement, resolved)
        || tag_only_payload_enum_if_is_statement_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

fn tag_only_payload_enum_if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        return false;
    };
    if symbol.kind != TypeSymbolKind::Enum
        || symbol.variants.len() > 256
        || symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    let Some(variant) = symbol
        .variants
        .iter()
        .find(|variant| variant.name == statement.variant_name)
    else {
        return false;
    };
    if !tag_only_if_is_payload_pattern_statement_is_buildable(
        statement,
        variant.payload.len(),
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return false;
    }
    payload_enum_pattern_target_expression_is_buildable(
        &statement.expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn tag_only_if_is_payload_pattern_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    payload_len: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match (statement.payload.as_ref(), payload_len) {
        (None, 0) | (Some(SwitchPayloadPattern::Discard(_)), 1) => true,
        (Some(SwitchPayloadPattern::Binding(binding)), 1) => payload_binding_is_buildable(
            binding,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn tag_only_payload_pattern_is_buildable(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match (payload, payload_len) {
        (None, 0) | (Some(SwitchPayloadPattern::Discard(_)), 1) => true,
        (Some(SwitchPayloadPattern::Binding(binding)), 1) => payload_binding_is_buildable(
            binding,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn tag_only_payload_pattern_covers_variant(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

fn payload_binding_is_buildable(
    binding: &crate::ast::SwitchPayloadBinding,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.binding_type_expr(binding.span) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    payload_if_is_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
}

fn payload_if_is_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, source_resolver)
    else {
        return false;
    };
    matches!(
        value.ty,
        AbiType::I32
            | AbiType::U8
            | AbiType::Usize
            | AbiType::Bool
            | AbiType::StrView
            | AbiType::SliceView
    ) || payload_binding_type_expr_is_supported_copy_aggregate(
        ty,
        &value,
        fallback_resolved,
        resolved_sources,
    )
}

fn payload_binding_type_expr_is_supported_copy_aggregate(
    ty: &TypeExpr,
    value: &AbiValue,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    if !abi_value_is_supported_aggregate_value(value) {
        return false;
    }
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_runtime_copy_value_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
        &mut HashSet::new(),
    )
}

fn switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || tag_only_payload_enum_switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn payloadless_switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return statement.wildcard_arm.is_some()
            && switch_target_payloadless_enum_symbol(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .is_some();
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

fn tag_only_payload_enum_switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.expression_type_expr(statement.expression.span()) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    if !type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources) {
        return false;
    }
    if !payload_enum_pattern_target_expression_shape_is_buildable(
        &statement.expression,
        typecheck_facts,
    ) {
        return false;
    }

    let Some(first_arm) = statement.arms.first() else {
        let source_resolver = |source| resolved_sources.get(&source).copied();
        return statement.wildcard_arm.is_some()
            && payload_enum_symbol_for_type_expr(&ty, resolved, &source_resolver).is_some();
    };

    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let Some(variant) = target_symbol
            .variants
            .iter()
            .find(|variant| variant.name == arm.variant_name)
        else {
            return false;
        };
        tag_only_payload_pattern_is_buildable(
            arm.payload.as_ref(),
            variant.payload.len(),
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn payload_enum_pattern_target_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.expression_type_expr(expression.span()) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        && payload_enum_pattern_target_expression_shape_is_buildable(expression, typecheck_facts)
}

fn payload_enum_pattern_target_expression_shape_is_buildable(
    expression: &Expr,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) | Expr::Call(_) => true,
        Expr::Member(member) => typecheck_facts
            .enum_variant_target(member.member_span)
            .is_some(),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            matches!(unwrap_group_expr(&unary.operand), Expr::Identifier(_))
        }
        _ => false,
    }
}

fn switch_target_payloadless_enum_symbol<'a>(
    statement: &crate::ast::SwitchStmt,
    resolved: &'a ResolveOutput,
    resolved_sources: &ResolvedSources<'a>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<&'a TypeSymbol> {
    let ty = typecheck_facts.expression_type_expr(statement.expression.span())?;
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    payloadless_enum_symbol_for_type_expr(&ty, resolved, &source_resolver)
}

fn payloadless_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payloadless,
    )
}

fn payload_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payload,
    )
}

#[derive(Clone, Copy)]
enum EnumPayloadRequirement {
    Payloadless,
    Payload,
}

fn enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr_inner(
        ty,
        fallback_resolved,
        resolver,
        payload_requirement,
        &mut HashSet::new(),
    )
}

fn enum_symbol_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
    resolving_names: &mut HashSet<String>,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            (generic.name.as_str(), substitutions)
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => return None,
    };
    let symbol = type_symbol_by_reference_name(resolved, type_name)?;
    if symbol.kind == TypeSymbolKind::Alias {
        let target = symbol.alias_target.as_ref()?;
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let result = enum_symbol_for_type_expr_inner(
            &target,
            fallback_resolved,
            resolver,
            payload_requirement,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return result;
    }

    enum_symbol_matches_payload_requirement(symbol, payload_requirement).then_some(symbol)
}

fn enum_symbol_matches_payload_requirement(
    symbol: &TypeSymbol,
    payload_requirement: EnumPayloadRequirement,
) -> bool {
    if symbol.kind != TypeSymbolKind::Enum || symbol.variants.len() > 256 {
        return false;
    }

    match payload_requirement {
        EnumPayloadRequirement::Payloadless => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        EnumPayloadRequirement::Payload => symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty()),
    }
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
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    let covered = statement
        .arms
        .iter()
        .filter_map(|arm| {
            let pattern_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
            if pattern_symbol.kind != TypeSymbolKind::Enum
                || pattern_symbol.canonical_name != target_symbol.canonical_name
                || arm.payload.is_some()
            {
                return None;
            }

            target_symbol
                .variants
                .iter()
                .find(|variant| variant.name == arm.variant_name)
                .map(|variant| variant.name.as_str())
        })
        .collect::<HashSet<_>>();
    target_symbol
        .variants
        .iter()
        .all(|variant| covered.contains(variant.name.as_str()))
}

fn switch_statement_covers_all_tag_only_payload_variants(
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
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    let covered = statement
        .arms
        .iter()
        .filter_map(|arm| {
            let arm_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
            if arm_symbol.kind != TypeSymbolKind::Enum
                || arm_symbol.canonical_name != target_symbol.canonical_name
            {
                return None;
            }
            let variant = target_symbol
                .variants
                .iter()
                .find(|variant| variant.name == arm.variant_name)?;
            tag_only_payload_pattern_covers_variant(arm.payload.as_ref(), variant.payload.len())
                .then_some(variant.name.as_str())
        })
        .collect::<HashSet<_>>();

    target_symbol
        .variants
        .iter()
        .all(|variant| covered.contains(variant.name.as_str()))
}

fn collect_statement_diagnostics(
    statement: &Stmt,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
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
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            let binding_is_fixed_array_literal = fixed_array_literal_binding_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_is_scalar_or_view = binding_initializer_may_use_value_control_expression(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_type_expr = binding_type_expr_with_substitutions(
                statement,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_fixed_array_type = binding_type_expr.as_ref().and_then(|ty| {
                fixed_array_type_abi_for_sources(ty, resolved, resolved_sources).map(|_| ty)
            });
            if let Expr::Otherwise(expression) = unwrap_group_expr(&statement.initializer) {
                collect_otherwise_binding_initializer_diagnostics(
                    expression,
                    binding_is_scalar_or_view,
                    binding_fixed_array_type,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if binding_is_fixed_array_literal
                || (binding_fixed_array_type.is_some()
                    && matches!(
                        unwrap_group_expr(&statement.initializer),
                        Expr::ArrayLiteral(_)
                    ))
            {
                collect_fixed_array_literal_binding_diagnostics(
                    statement,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if binding_is_scalar_or_view {
                collect_value_expression_diagnostics(
                    &statement.initializer,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                    resolved_sources,
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
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.operator_span,
                    "compound assignment statements",
                    "use `i32`, `usize`, or `u8` whole-binding, aggregate-field, read-write slice element, or local/aggregate-field fixed-array element compound assignment, or use `target = target op value` until broader compound assignment lowering is promoted",
                ));
            }
            if let Some(diagnostic) = unsupported_index_assignment_target_diagnostic(
                sources,
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_fixed_array_assignment_diagnostic(
                sources,
                statement,
                resolved,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            let assignment_is_fixed_array_literal = fixed_array_literal_assignment_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let assignment_targets_fixed_array = fixed_array_assignment_target_abi(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .is_some();
            let assignment_aggregate_type = aggregate_assignment_target_type_expr(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let assignment_is_scalar_or_view = assignment_value_may_use_value_control_expression(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            if let Expr::Otherwise(otherwise) = unwrap_group_expr(&statement.value)
                && (assignment_is_scalar_or_view || assignment_aggregate_type.is_some())
            {
                collect_otherwise_assignment_value_diagnostics(
                    otherwise,
                    assignment_aggregate_type.as_ref(),
                    assignment_is_scalar_or_view,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if assignment_is_scalar_or_view {
                collect_value_expression_diagnostics(
                    &statement.value,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if assignment_is_fixed_array_literal
                || (assignment_targets_fixed_array
                    && matches!(unwrap_group_expr(&statement.value), Expr::ArrayLiteral(_)))
            {
                collect_fixed_array_literal_elements_diagnostics(
                    unwrap_group_expr(&statement.value),
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::If(statement) => {
            let exits_function = if_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            );
            if exits_function {
                collect_terminal_control_condition_move_diagnostics(
                    &statement.condition,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
            } else {
                collect_nonterminal_control_block_aggregate_diagnostics(
                    &statement.then_block,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        block,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
                collect_control_condition_move_diagnostics(
                    &statement.condition,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
            }
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::IfIs(statement) => {
            if !if_is_statement_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.pattern_span,
                    "`if is` pattern branches",
                    "use payloadless enum patterns or tag-only payload enum patterns over existing values and supported call/constructor/move-local pattern targets, or keep unsupported payload binding code on the `check` path",
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if !if_is_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                collect_nonterminal_control_payload_block_aggregate_diagnostics(
                    &statement.then_block,
                    statement
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        block,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
            }
            collect_if_is_target_move_diagnostics(
                statement,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Switch(statement) => {
            if !switch_statement_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "`match` statements",
                    "use payloadless enum `match` arms or tag-only payload enum discard arms over existing values, or keep payload binding code on the `check` path",
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if !switch_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                for arm in &statement.arms {
                    collect_nonterminal_control_payload_block_aggregate_diagnostics(
                        &arm.body,
                        arm.payload
                            .as_ref()
                            .and_then(|payload| payload.binding_name()),
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
                if let Some(arm) = &statement.wildcard_arm {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        &arm.body,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
            }
            collect_switch_target_move_diagnostics(
                statement,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            for arm in &statement.arms {
                collect_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::While(statement) => {
            collect_control_condition_move_diagnostics(
                &statement.condition,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Loop(statement) => {
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn collect_control_condition_move_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(span) = expression_explicit_aggregate_move_span(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return;
    };

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "select the branch before moving aggregate values until control-flow condition move lowering is promoted",
    ));
}

fn collect_if_is_target_move_diagnostics(
    statement: &crate::ast::IfIsStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if if_is_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return;
    }

    collect_control_condition_move_diagnostics(
        &statement.expression,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
}

fn collect_switch_target_move_diagnostics(
    statement: &crate::ast::SwitchStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return;
    }

    collect_control_condition_move_diagnostics(
        &statement.expression,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
}

fn collect_terminal_control_condition_move_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(span) = expression_explicit_aggregate_move_span(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return;
    };
    if condition_explicit_moves_are_single_evaluation_call_for_buildability(expression) {
        return;
    }

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "use a single call expression for terminal branch conditions that move aggregate values, or move aggregate values after branch selection until broader condition move lowering is promoted",
    ));
}

fn condition_explicit_moves_are_single_evaluation_call_for_buildability(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&unary.operand)
        }
        Expr::Propagate(propagation) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(
                &propagation.expression,
            )
        }
        Expr::Force(force) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&force.expression)
        }
        Expr::Catch(catch) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&catch.expression)
        }
        _ => false,
    }
}

fn collect_nonterminal_control_block_aggregate_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
        block,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        HashSet::new(),
        diagnostics,
    );
}

fn collect_nonterminal_control_payload_block_aggregate_diagnostics(
    block: &Block,
    payload_name: Option<&str>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_bindings = HashSet::new();
    if let Some(payload_name) = payload_name {
        local_bindings.insert(payload_name.to_owned());
    }
    collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
        block,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
        diagnostics,
    );
}

fn collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    mut local_bindings: HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Stmt::Binding(statement) => {
                if let Some(span) = unsupported_outer_aggregate_move_binding_span(
                    statement,
                    statements,
                    index,
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or move outer values only into bindings/assignments on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
                local_bindings.insert(statement.name.clone());
            }
            Stmt::Assignment(statement) => {
                if let Some(span) = unsupported_outer_aggregate_move_assignment_span(
                    statement,
                    statements,
                    index,
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or move outer values only into bindings/assignments on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
            }
            Stmt::Expression(statement) => {
                if let Some(span) = expression_explicit_outer_aggregate_move_span(
                    &statement.expression,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or bind or assign outer moves only on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
            }
            Stmt::Drop(statement)
                if !local_bindings.contains(&statement.name)
                    && !statement_suffix_exits_function_for_buildability(
                        statements,
                        index,
                        result,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                    ) =>
            {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "explicit outer aggregate drops inside non-terminal control flow",
                    "drop values created inside the branch/body, or drop outer values only on paths that immediately exit the function until broader control-flow drop lowering is promoted",
                ));
            }
            _ => {}
        }
    }
    if let Some(result) = result
        && let Some(span) = expression_explicit_outer_aggregate_move_span(
            result,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            &local_bindings,
        )
    {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            span,
            "explicit outer aggregate moves inside non-terminal control-flow results",
            "move values created inside the branch/body, or move outer values only before a statement that immediately exits the function until broader control-flow move lowering is promoted",
        ));
    }
}

fn unsupported_outer_aggregate_move_binding_span(
    statement: &BindingStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if direct_outer_aggregate_move_for_buildability(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }
    Some(span)
}

fn unsupported_outer_aggregate_move_assignment_span(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
        statement,
        statements,
        index,
        result,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) {
        return None;
    }
    Some(span)
}

fn assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    direct_outer_aggregate_move_for_buildability(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && assignment_target_root_is_aggregate_binding_for_buildability(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn direct_outer_aggregate_move_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    let Expr::Unary(unary) = unwrap_group_expr(expression) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) else {
        return false;
    };
    identifier_is_outer_aggregate_for_buildability(
        identifier,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )
}

#[derive(Clone, Copy)]
enum ExplicitAggregateMoveScope<'a> {
    Any,
    OutsideLocals(&'a HashSet<String>),
}

fn expression_explicit_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::Any,
    )
}

fn expression_explicit_outer_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings),
    )
}

fn explicit_aggregate_move_span_in_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) {
                explicit_aggregate_move_matches_identifier(
                    identifier,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
                .then_some(unary.span)
            } else {
                explicit_aggregate_move_span_in_expression(
                    &unary.operand,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            }
        }
        Expr::ArrayLiteral(literal) => literal.elements.iter().find_map(|element| {
            explicit_aggregate_move_span_in_expression(
                element,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::StructLiteral(literal) => literal.fields.iter().find_map(|field| {
            explicit_aggregate_move_span_in_expression(
                &field.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Propagate(propagation) => explicit_aggregate_move_span_in_expression(
            &propagation.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Force(force) => explicit_aggregate_move_span_in_expression(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Catch(catch) => explicit_aggregate_move_span_in_expression(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Borrow(borrow) => explicit_aggregate_move_span_in_expression(
            &borrow.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Unary(unary) => explicit_aggregate_move_span_in_expression(
            &unary.operand,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Binary(binary) => explicit_aggregate_move_span_in_expression(
            &binary.left,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &binary.right,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::TypeConversion(conversion) => explicit_aggregate_move_span_in_expression(
            &conversion.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Call(call) => explicit_aggregate_move_span_in_expression(
            &call.callee,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            call.arguments.iter().find_map(|argument| {
                explicit_aggregate_move_span_in_expression(
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Member(member) => explicit_aggregate_move_span_in_expression(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Index(index) => explicit_aggregate_move_span_in_expression(
            &index.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &index.index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Group(group) => explicit_aggregate_move_span_in_expression(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Otherwise(expression) => explicit_aggregate_move_span_in_expression(
            &expression.value,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &expression.fallback,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Match(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().find_map(|part| {
            if let InterpolatedStringPart::Expression(part) = part {
                explicit_aggregate_move_span_in_expression(
                    &part.expression,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            } else {
                None
            }
        }),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn explicit_aggregate_move_span_in_block(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => block
            .statements
            .iter()
            .find_map(|statement| {
                explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
            .or_else(|| {
                block.result.as_ref().and_then(|result| {
                    explicit_aggregate_move_span_in_expression(
                        result,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        scope,
                    )
                })
            }),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut nested_locals = local_bindings.clone();
            for statement in &block.statements {
                let span = explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                );
                if span.is_some() {
                    return span;
                }
                if let Stmt::Binding(statement) = statement {
                    nested_locals.insert(statement.name.clone());
                }
            }
            block.result.as_ref().and_then(|result| {
                explicit_aggregate_move_span_in_expression(
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                )
            })
        }
    }
}

fn explicit_aggregate_move_span_in_statement(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => None,
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            explicit_aggregate_move_span_in_expression(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Binding(statement) => explicit_aggregate_move_span_in_expression(
            &statement.initializer,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Assignment(statement) => explicit_aggregate_move_span_in_expression(
            &statement.target,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::Switch(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::ForRange(statement) => explicit_aggregate_move_span_in_expression(
            &statement.start,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.end,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            explicit_aggregate_move_span_in_for_range_body(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::While(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Loop(statement) => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Expression(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

fn explicit_aggregate_move_span_in_for_range_body(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut body_locals = local_bindings.clone();
            body_locals.insert(statement.name.clone());
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&body_locals),
            )
        }
    }
}

fn explicit_aggregate_move_span_in_payload_block(
    block: &Block,
    payload_name: Option<&str>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match (scope, payload_name) {
        (ExplicitAggregateMoveScope::OutsideLocals(local_bindings), Some(payload_name)) => {
            let mut nested_locals = local_bindings.clone();
            nested_locals.insert(payload_name.to_owned());
            explicit_aggregate_move_span_in_block(
                block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
            )
        }
        _ => explicit_aggregate_move_span_in_block(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

fn explicit_aggregate_move_matches_identifier(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> bool {
    match scope {
        ExplicitAggregateMoveScope::Any => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            identifier_is_outer_aggregate_for_buildability(
                identifier,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                local_bindings,
            )
        }
    }
}

fn assignment_target_root_is_aggregate_binding_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Member(member) => assignment_target_root_is_aggregate_binding_for_buildability(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn identifier_is_outer_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    !local_bindings.contains(&identifier.name)
        && identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

fn identifier_is_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
        return false;
    };
    let Some(ty) = typecheck_facts.binding_type_expr(symbol.name_span) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    type_expr_is_supported_aggregate_value_for_sources(&ty, resolved, resolved_sources)
}

fn statement_suffix_exits_function_for_buildability(
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    statement_sequence_or_result_exits_function_for_buildability(
        statements.get(index + 1..).unwrap_or(&[]),
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn statement_sequence_or_result_exits_function_for_buildability(
    statements: &[Stmt],
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop_for_buildability(statement) {
            return false;
        }
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ) {
            return true;
        }
    }
    result.is_some_and(|expression| {
        expression_exits_function_for_buildability(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn statement_exits_function_for_buildability(
    statement: &Stmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function_for_buildability(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Switch(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn if_statement_exits_function_for_buildability(
    statement: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn if_is_statement_exits_function_for_buildability(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn switch_statement_exits_function_for_buildability(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.wildcard_arm.is_none()
        && !switch_statement_covers_all_payloadless_variants(statement, resolved)
        && !switch_statement_covers_all_tag_only_payload_variants(statement, resolved)
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        block_exits_function_for_buildability(
            &arm.body,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    }) && statement.wildcard_arm.as_ref().is_none_or(|wildcard_arm| {
        block_exits_function_for_buildability(
            &wildcard_arm.body,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

fn block_exits_function_for_buildability(
    block: &Block,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    statement_sequence_or_result_exits_function_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

fn expression_exits_function_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typecheck_facts, generic_substitutions),
            Some(ReturnShape::Never)
        ),
        Expr::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

fn statement_may_exit_current_loop_for_buildability(statement: &Stmt) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        Stmt::While(_) | Stmt::Loop(_) => false,
        _ => false,
    }
}

fn block_may_exit_current_loop_for_buildability(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(statement_may_exit_current_loop_for_buildability)
        || block
            .result
            .as_deref()
            .is_some_and(expression_may_exit_current_loop_for_buildability)
}

fn expression_may_exit_current_loop_for_buildability(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::Match(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        _ => false,
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if expression_statement_is_supported(
        expression,
        resolved,
        resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape_for_sources(
                call,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
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
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::StructLiteral(literal) => aggregate_literal_statement_is_supported(literal, resolved),
        _ => false,
    }
}

fn catch_block_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return false;
    }

    let Some((last, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        catch_block_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && catch_block_terminal_statement_runtime_shape_is_buildable(
        last,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn catch_block_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) | Stmt::Binding(_) | Stmt::Assignment(_) => true,
        Stmt::Expression(statement) => expression_statement_is_supported(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Return(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

fn catch_block_terminal_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => {
            expression_exits_function_for_buildability(
                &statement.expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) || expression_statement_is_supported(
                &statement.expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

fn otherwise_optional_value_call_is_buildable(
    value: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::Call(call) = unwrap_group_expr(value) else {
        return false;
    };
    let Some(return_type) = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_top_level_optional_with_resolver(&return_type, resolved, &source_resolver)
}

fn otherwise_return_fallback_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return block.statements.iter().all(|statement| {
            otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        });
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && match terminal {
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_is_never_runtime_shape_is_buildable(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

fn otherwise_binding_fallback_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return block.statements.iter().all(|statement| {
            otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        });
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && match terminal {
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Expression(statement) => expression_is_never_runtime_shape_is_buildable(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Drop(_) => false,
    }
}

fn otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::Drop(_) => true,
        Stmt::Expression(statement) => expression_statement_is_supported(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Return(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

fn otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn expression_is_never_runtime_shape_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typecheck_facts, generic_substitutions),
            Some(ReturnShape::Never)
        ),
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape_for_sources(
                call,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
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
        typecheck_facts.binding_scalar_view_kind(statement.name_span),
        Some(TypecheckScalarViewKind::I32 | TypecheckScalarViewKind::Usize)
    )
}

fn assignment_operator_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
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
                typecheck_facts.binding_scalar_view_kind(symbol.name_span),
                Some(
                    TypecheckScalarViewKind::I32
                        | TypecheckScalarViewKind::Usize
                        | TypecheckScalarViewKind::U8
                )
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typecheck_facts)
        }
        Expr::Index(index) => {
            fixed_array_index_compound_assignment_is_buildable(
                index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) || slice_index_compound_assignment_is_buildable(
                &index.object,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => false,
    }
}

fn fixed_array_index_compound_assignment_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some((element, layout)) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) else {
        return false;
    };
    layout.size > 0 && matches!(element, AbiType::I32 | AbiType::U8 | AbiType::Usize)
}

fn slice_index_compound_assignment_is_buildable(
    object: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    matches!(
        slice_index_assignment_element_kind(
            object,
            resolved,
            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    let Expr::Index(index) = unwrap_group_expr(&statement.target) else {
        return None;
    };
    if let Some(is_buildable) = fixed_array_index_assignment_target_is_buildable(
        index,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_v0_build_diagnostic(
            sources,
            index.span,
            "fixed array index assignment targets outside scalar/view element locals or aggregate fields",
            "assign through an index into a local or aggregate-field `[i32; N]`, `[u8; N]`, `[usize; N]`, `[bool; N]`, or `[&str; N]` until broader fixed array mutation is promoted",
        ));
    }
    if matches!(
        slice_index_assignment_target_is_buildable(
            &index.object,
            resolved,
            resolved_sources,
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

fn fixed_array_index_assignment_target_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    let (element, layout) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    )?;
    Some(layout.size > 0 && fixed_array_element_abi_is_buildable(&element))
}

fn slice_index_assignment_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Slice(element) => {
                    if typecheck_slice_element_kind_is_buildable(element) {
                        return Some(true);
                    }
                    let ty = typecheck_facts.binding_type_expr(symbol.name_span)?;
                    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
                    slice_index_target_type_expr_is_buildable_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
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
            slice_index_target_type_expr_is_buildable_for_sources(
                &return_type,
                resolved,
                resolved_sources,
            )
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Slice(element) => {
                if typecheck_slice_element_kind_is_buildable(element) {
                    return Some(true);
                }
                let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
                let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
                slice_index_target_type_expr_is_buildable_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
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
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_target_is_buildable(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_target_is_buildable(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_target_is_buildable(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn slice_index_assignment_fallible_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
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
    slice_index_target_type_expr_is_buildable_for_sources(
        &fallible.success,
        resolved,
        resolved_sources,
    )
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

fn call_return_shape_for_sources(
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ReturnShape> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    Some(return_shape_from_type_expr_for_sources(
        &return_type,
        resolved,
        resolved_sources,
    ))
}

fn return_shape_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> ReturnShape {
    return_shape_from_type_expr_with_resolver(ty, resolved, &|_| Some(resolved))
}

fn return_shape_from_type_expr_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> ReturnShape {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    return_shape_from_type_expr_with_resolver(ty, fallback_resolved, &source_resolver)
}

fn return_shape_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> ReturnShape
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    return_shape_from_type_expr_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn return_shape_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> ReturnShape
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => ReturnShape::Void,
        TypeExpr::Reference(reference) if reference.name == "never" => ReturnShape::Never,
        TypeExpr::Reference(reference)
            if matches!(reference.name.as_str(), "i32" | "u8" | "usize" | "bool") =>
        {
            ReturnShape::DiscardableScalar
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            ReturnShape::DiscardableView
        }
        TypeExpr::Borrow(borrow)
            if type_expr_resolves_to_supported_slice_view_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            )
            .unwrap_or(false) =>
        {
            ReturnShape::DiscardableView
        }
        _ if type_expr_is_supported_aggregate_return_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ) =>
        {
            ReturnShape::DiscardableAggregate
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return ReturnShape::Other;
            };
            let Some(target) = &symbol.alias_target else {
                return ReturnShape::Other;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return ReturnShape::Other;
            }
            let shape = return_shape_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            shape
        }
        TypeExpr::Fallible(fallible) => {
            match return_shape_from_type_expr_inner(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            ) {
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

fn type_expr_is_supported_aggregate_return_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
}

fn collect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
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
                        resolved_sources,
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
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                if fixed_array_literal_struct_field_has_fixed_array_type(
                    field,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_fixed_array_literal_elements_diagnostics(
                        unwrap_group_expr(&field.value),
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if let Some(field_type) = otherwise_aggregate_struct_field_type(
                    field,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    let Expr::Otherwise(otherwise) = unwrap_group_expr(&field.value) else {
                        unreachable!("aggregate otherwise field helper checked expression shape");
                    };
                    collect_otherwise_aggregate_value_expression_diagnostics(
                        otherwise,
                        &field_type,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if struct_literal_field_may_use_value_control_expression(
                    field.name_span,
                    typecheck_facts,
                ) {
                    collect_value_expression_diagnostics(
                        &field.value,
                        None,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
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
                        resolved_sources,
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
            resolved_sources,
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
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Catch(expression) => {
            if !catch_block_runtime_shape_is_buildable(
                &expression.catch_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    expression.catch_block.span,
                    "`catch` blocks outside the v0 runtime subset",
                    "end runtime-shipped `catch` blocks with a direct `return` or supported effect-only/never expression statement until broader catch control-flow lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.catch_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
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
            resolved_sources,
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
            resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
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
            resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_method_borrow_receiver_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
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
            if !payload_enum_constructor_call_is_supported(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                collect_expression_diagnostics(
                    &expression.callee,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
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
                if fixed_array_literal_argument_has_fixed_array_parameter_type(
                    expression,
                    index,
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_fixed_array_literal_elements_diagnostics(
                        unwrap_group_expr(argument),
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if let Some(parameter_type) = otherwise_aggregate_argument_parameter_type(
                    expression,
                    index,
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    let Expr::Otherwise(otherwise) = unwrap_group_expr(argument) else {
                        unreachable!(
                            "aggregate otherwise argument helper checked expression shape"
                        );
                    };
                    collect_otherwise_aggregate_value_expression_diagnostics(
                        otherwise,
                        &parameter_type,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if call_argument_may_use_value_control_expression(
                    expression,
                    index,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_value_expression_diagnostics(
                        argument,
                        None,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
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
                        resolved_sources,
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
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_field_member_value_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(root_type) = otherwise_aggregate_member_root_type(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                let otherwise = aggregate_member_root_otherwise(&expression.object)
                    .expect("aggregate otherwise member helper checked expression shape");
                collect_otherwise_aggregate_value_expression_diagnostics(
                    otherwise,
                    &root_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &expression.object,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Expr::Index(expression) => {
            if let Some(diagnostic) = unsupported_slice_index_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                resolved_sources,
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
                resolved_sources,
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
                resolved_sources,
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
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Otherwise(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions",
                "use `otherwise` directly as a scalar/view value, aggregate member access root, aggregate argument, aggregate field initializer, binding initializer, assignment value, or return expression until general optional expression lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.value,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.fallback,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
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
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_block_diagnostics(
                    &arm.body,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_block_diagnostics(
                    &wildcard_arm.body,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
    }
}

fn collect_fixed_array_literal_binding_diagnostics(
    statement: &BindingStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_fixed_array_literal_elements_diagnostics(
        unwrap_group_expr(&statement.initializer),
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn collect_fixed_array_literal_elements_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::ArrayLiteral(literal) = expression else {
        collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
        return;
    };

    for element in &literal.elements {
        collect_value_expression_diagnostics(
            element,
            None,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

fn unsupported_field_member_value_diagnostic(
    sources: &SourceMap,
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
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
    let field_ty = substitute_type_expr_parameters(&field_ty, generic_substitutions);
    match member_field_value_type_is_buildable(&field_ty, resolved, resolved_sources)? {
        true => None,
        false => Some(unsupported_v0_build_diagnostic(
            sources,
            expression.member_span,
            "field member values outside supported scalar/view or aggregate types",
            "keep `u16`, `u32`, and other storage-only fields encapsulated in aggregates, or expose an `i32`, `usize`, or `u8` value until broader scalar field lowering is promoted",
        )),
    }
}

fn field_type_expr_for_member(
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    field_type_expr_for_span(expression.member_span, resolved, typecheck_facts)
}

fn field_type_expr_for_span(
    field_span: ByteSpan,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    if let Some(ty) = typecheck_facts.field_type_expr(field_span) {
        return Some(ty.clone());
    }
    let target_span = typecheck_facts.field_target(field_span)?;
    resolved.symbols.symbols().find_map(|symbol| {
        let SymbolKind::Type(type_symbol) = &symbol.kind else {
            return None;
        };
        type_symbol
            .fields
            .iter()
            .find(|field| field.name_span == target_span)
            .map(|field| field.ty.clone())
    })
}

fn member_field_value_type_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    if type_expr_contains_unresolved_type_parameter(ty, resolved, resolved_sources) {
        return None;
    }
    if type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
    {
        return Some(true);
    }
    Some(false)
}

fn type_expr_contains_unresolved_type_parameter(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_contains_unresolved_type_parameter_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}

fn type_expr_contains_unresolved_type_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            !known_builtin_type_name(&reference.name)
                && type_symbol_by_reference_name(resolved, &reference.name).is_none()
        }
        TypeExpr::Generic(generic) => generic.arguments.iter().any(|argument| {
            type_expr_contains_unresolved_type_parameter_with_resolver(
                argument,
                fallback_resolved,
                resolver,
            )
        }),
        TypeExpr::Pointer(pointer) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &pointer.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Borrow(borrow) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &borrow.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::View(view) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Array(array) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &array.element,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Optional(optional) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_unresolved_type_parameter_with_resolver(
                &fallible.success,
                fallback_resolved,
                resolver,
            ) || type_expr_contains_unresolved_type_parameter_with_resolver(
                &fallible.error,
                fallback_resolved,
                resolver,
            )
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
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    // `std/vec` generic bodies keep parameter element facts as `Other`; user
    // call sites are preflighted before those bodies are lowered.
    if source_is_std_vec(sources, expression.span.source, nocter_home) {
        return None;
    }

    if let Some(is_buildable) = fixed_array_index_expression_is_buildable(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_v0_build_diagnostic(
            sources,
            expression.span,
            "fixed array indexing outside scalar/view element local or aggregate-field reads",
            "index a local or aggregate-field `[i32; N]`, `[u8; N]`, `[usize; N]`, `[bool; N]`, or `[&str; N]` value until broader fixed array indexing is promoted",
        ));
    }

    if slice_index_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
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

fn fixed_array_index_expression_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    let (element, layout) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    )?;
    Some(layout.size > 0 && fixed_array_element_abi_is_buildable(&element))
}

fn fixed_array_index_target_abi(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, crate::abi::ValueLayout)> {
    let ty = fixed_array_index_target_type_expr(
        &expression.object,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let (element, _length, layout) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)?;
    Some((element, layout))
}

fn fixed_array_index_target_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            typecheck_facts
                .binding_type_expr(symbol.name_span)
                .cloned()
                .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
        }
        Expr::Member(member) => field_type_expr_for_member(member, resolved, typecheck_facts)
            .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions)),
        Expr::Group(group) => fixed_array_index_target_type_expr(
            &group.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn slice_index_expression_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    slice_index_target_is_buildable(
        &expression.object,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn slice_index_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
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
                    if typecheck_slice_element_kind_is_buildable(element) {
                        return Some(true);
                    }
                    let ty = typecheck_facts.binding_type_expr(symbol.name_span)?;
                    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
                    slice_index_target_type_expr_is_buildable_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
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
            slice_index_target_type_expr_is_buildable_for_sources(
                &return_type,
                resolved,
                resolved_sources,
            )
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Str => Some(true),
            TypecheckScalarViewKind::Slice(element) => {
                if typecheck_slice_element_kind_is_buildable(element) {
                    return Some(true);
                }
                let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
                let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
                slice_index_target_type_expr_is_buildable_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
            }
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool => None,
        },
        Expr::Group(group) => slice_index_target_is_buildable(
            &group.expression,
            resolved,
            resolved_sources,
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

fn type_expr_is_supported_slice_index_element_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_slice_element_kind_with_resolver(ty, fallback_resolved, resolver)
        != TypecheckSliceElementKind::Other
        || type_expr_is_supported_copy_aggregate_vec_element_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        )
}

fn slice_index_assignment_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypecheckSliceElementKind> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
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
            slice_index_target_type_expr_element_kind_with_resolver(
                &return_type,
                resolved,
                &source_resolver,
            )
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
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_element_kind(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_element_kind(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_element_kind(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

fn slice_index_assignment_fallible_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
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
    let source_resolver = |source| resolved_sources.get(&source).copied();
    slice_index_target_type_expr_element_kind_with_resolver(
        &fallible.success,
        resolved,
        &source_resolver,
    )
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

fn slice_index_target_type_expr_is_buildable_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    slice_index_target_type_expr_is_buildable_with_resolver(ty, fallback_resolved, &source_resolver)
}

fn slice_index_target_type_expr_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    slice_index_target_type_expr_is_buildable_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn slice_index_target_type_expr_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            Some(true)
        }
        TypeExpr::Borrow(borrow) => type_expr_resolves_to_supported_slice_view_with_resolver(
            &borrow.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = slice_index_target_type_expr_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn slice_index_target_type_expr_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    slice_index_target_type_expr_element_kind_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn slice_index_target_type_expr_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                )
            {
                return Some(TypecheckSliceElementKind::Str);
            }
            type_expr_resolved_view_element_kind_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            )
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = slice_index_target_type_expr_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
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
    resolved_sources: &ResolvedSources<'_>,
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
    if type_expr_is_supported_std_vec_element_storage(&element, resolved, resolved_sources) {
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
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

    if typecheck_facts
        .expression_type_expr(member.span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_some_and(|ty| {
            type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        })
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

fn payload_enum_constructor_call_is_supported(
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Member(member) = call.callee.as_ref() else {
        return false;
    };
    if typecheck_facts
        .enum_variant_target(member.member_span)
        .is_none()
    {
        return false;
    }
    typecheck_facts
        .expression_type_expr(call.span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_some_and(|ty| {
            type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        })
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
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if type_expr_slice_element_kind_with_resolver(ty, fallback_resolved, &source_resolver)
        != TypecheckSliceElementKind::Other
    {
        return true;
    }

    type_expr_is_supported_copy_aggregate_vec_element_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}

fn type_expr_is_supported_copy_aggregate_vec_element_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver) else {
        return false;
    };
    if !matches!(value.ty, AbiType::Struct(_)) || value.layout.size == 0 {
        return false;
    }
    type_expr_is_runtime_copy_struct_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn type_expr_is_runtime_copy_struct_with_resolver<'a, F>(
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
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            if symbol.generic_arity > 0 {
                return false;
            }
            type_symbol_is_runtime_copy_struct_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &HashMap::new(),
                resolving_names,
            )
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
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
            type_symbol_is_runtime_copy_struct_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Fallible(fallible) => type_expr_is_runtime_copy_struct_with_resolver(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_runtime_copy_struct_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        _ => false,
    }
}

fn type_symbol_is_runtime_copy_struct_with_resolver<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }

    let is_copy = match symbol.kind {
        TypeSymbolKind::Struct if !symbol.is_copy => false,
        TypeSymbolKind::Struct => symbol.fields.iter().all(|field| {
            let field_ty = substitute_type_expr_parameters(&field.ty, substitutions);
            type_expr_is_runtime_copy_value_with_resolver(
                &field_ty,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }),
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
            let target = substitute_type_expr_parameters(target, substitutions);
            type_expr_is_runtime_copy_struct_with_resolver(
                &target,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }),
        TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
    };

    resolving_names.remove(&symbol.canonical_name);
    is_copy
}

fn type_expr_is_runtime_copy_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
            | "isize" | "error" => true,
            "str" | "void" | "never" | "Self" => false,
            _ => {
                let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
                let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                    return false;
                };
                if symbol.generic_arity > 0 {
                    return false;
                }
                type_symbol_is_runtime_copy_value_with_resolver(
                    symbol,
                    fallback_resolved,
                    resolver,
                    &HashMap::new(),
                    resolving_names,
                )
            }
        },
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
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
            type_symbol_is_runtime_copy_value_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Borrow(borrow) => !borrow.is_readwrite,
        TypeExpr::Pointer(_) => true,
        TypeExpr::Array(array) => type_expr_is_runtime_copy_value_with_resolver(
            &array.element,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_runtime_copy_value_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_is_runtime_copy_value_with_resolver(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            ) && type_expr_is_runtime_copy_value_with_resolver(
                &fallible.error,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::View(_) => false,
    }
}

fn type_symbol_is_runtime_copy_value_with_resolver<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match symbol.kind {
        TypeSymbolKind::Struct => type_symbol_is_runtime_copy_struct_with_resolver(
            symbol,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        ),
        TypeSymbolKind::Enum => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        TypeSymbolKind::Alias => {
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let is_copy = symbol.alias_target.as_ref().is_some_and(|target| {
                let target = substitute_type_expr_parameters(target, substitutions);
                type_expr_is_runtime_copy_value_with_resolver(
                    &target,
                    fallback_resolved,
                    resolver,
                    resolving_names,
                )
            });
            resolving_names.remove(&symbol.canonical_name);
            is_copy
        }
        TypeSymbolKind::Interface => false,
    }
}

fn type_expr_slice_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_slice_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_slice_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
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
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            TypecheckSliceElementKind::Str
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
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
            let kind = type_expr_slice_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
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
    if !expression_is_statically_zero_integer(argument) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "null raw pointer construction",
        "`*T` is non-null in v0; use `none` for `*T?` absence or pass a non-zero trusted address",
    ))
}

fn expression_is_statically_zero_integer(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value) == Some(0),
        Expr::TypeConversion(conversion) => {
            expression_is_statically_zero_integer(&conversion.expression)
        }
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
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
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
            if !type_expr_resolves_to_borrow_with_resolver(
                &parameter_ty,
                resolved,
                &source_resolver,
            ) {
                return None;
            }
            match unwrap_group_expr(argument) {
                Expr::Borrow(borrow)
                    if borrow.is_readwrite
                        && !readwrite_borrow_argument_source_is_buildable(
                            &borrow.expression,
                            resolved,
                            resolved_sources,
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
    resolved_sources: &ResolvedSources<'_>,
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
        resolved_sources,
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
    matches!(
        typecheck_facts.method_call_receiver_kind(member_span),
        Some(TypecheckMethodReceiverKind::ReadwriteBorrow)
    )
}

fn readwrite_borrow_argument_source_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
        Expr::Index(index) => slice_index_assignment_element_kind(
            &index.object,
            resolved,
            resolved_sources,
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

fn type_expr_resolves_to_borrow_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_borrow_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn type_expr_resolves_to_borrow_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(_) => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let resolves = type_expr_resolves_to_borrow_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
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
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    nested_fallible_return_type_issue(
        &return_type,
        function.return_type.span(),
        resolved,
        resolved_sources,
    )
}

fn nested_fallible_return_type_issue(
    return_type: &TypeExpr,
    diagnostic_span: ByteSpan,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    if type_expr_fallible_depth(return_type, resolved, resolved_sources) <= 1 {
        return None;
    }

    Some(BuildabilityIssue {
        span: diagnostic_span,
        construct: "nested fallible or optional return types",
        help: "flatten the return boundary to a single optional or fallible layer until nested fallible lowering is promoted",
    })
}

fn type_expr_fallible_depth(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> usize {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_fallible_depth_inner(ty, fallback_resolved, &source_resolver, &mut HashSet::new())
}

fn type_expr_fallible_depth_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> usize
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return 0;
            };
            let Some(target) = &symbol.alias_target else {
                return 0;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return 0;
            }
            let depth = type_expr_fallible_depth_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            depth
        }
        TypeExpr::Fallible(fallible) => {
            1 + type_expr_fallible_depth_inner(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::Optional(optional) => {
            1 + type_expr_fallible_depth_inner(
                &optional.inner,
                fallback_resolved,
                resolver,
                resolving_names,
            )
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

        _ {
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
    fn does_not_report_payloadless_wildcard_only_match() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func choose(): Choice {
    return Choice.no
}

func main(): i32 {
    let value = match choose() {
        _ { 7 }
    }
    match value_choice() {
        _ {
            return value
        }
    }
}

func value_choice(): Choice {
    return Choice.yes
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
        _ { 1 }
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
        _ { 1 }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_reachable_payload_match_expression_non_copy_binding_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    return match result {
        Result.ok(value) { value.code }
        _ { 0 }
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
    fn does_not_treat_if_is_payload_move_as_outer_control_move() {
        let (sources, analysis) = analyze_text(
            r#"struct File {
    fd: i32
}

enum Event {
    file(file: File)
    empty
}

func main(): i32 {
    let event = Event.file(File{ fd: 1 })
    if event is Event.file(file) {
        var moved = move file
    }
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("`if is` pattern branches")),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("explicit outer aggregate moves inside non-terminal control flow")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn does_not_report_payload_enum_if_is_constructor_pattern_target() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if Result.ok(42) is Result.ok(value) {
        return value
    }

    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_payload_enum_if_is_move_pattern_target() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(42)
    if move result is Result.ok(value) {
        return value
    }

    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_payload_enum_match_member_pattern_target() {
        let (sources, analysis) = analyze_text(
            r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    match Result.failed {
        Result.ok(_) {
            return 1
        }

        _ {
            return 42
        }
    }
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_copy_payload_enum_construction() {
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payload_enum_empty_variant_construction() {
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payload_enum_single_drop_payload_construction() {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload{ value: 10 })
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_payload_enum_multi_drop_payload_construction() {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload{ value: 10 }, Payload{ value: 20 })
    return 0
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_scope_drop_body_with_multi_field_payload_enum() {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let result = Result.ok(Payload{ value: 1 }, Payload{ value: 2 })
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_generic_scope_drop_body_with_multi_field_payload_enum() {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Box<T> {
    value: T
}

impl<T> Box<T> {
    drop &+self {
        let result = Result.ok(Payload{ value: 1 }, Payload{ value: 2 })
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_field_replacement_drop_body_with_multi_field_payload_enum() {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let result = Result.ok(Payload{ value: 1 }, Payload{ value: 2 })
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_reachable_generic_field_replacement_drop_body_with_multi_field_payload_enum()
    {
        let (sources, analysis) = analyze_text(
            r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let result = Result.ok(Payload{ value: 1 }, Payload{ value: 2 })
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

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_unreachable_tail_after_return() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return 0
    let bytes: [u8; 2] = [1, 2]
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn does_not_report_unreachable_tail_after_exhaustive_match_statement() {
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

        Choice.no {
            return 1
        }
    }
    let stored: u16 = 0 as u16
    return 2
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
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
    fn accepts_reachable_fixed_array_aggregate_field_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Bag {
    values: [i32; 2]
}

func main(): i32 {
    var bag = Bag{ values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Bag{ values: [5, 6] }
    bag.values = [7, 8]
    bag.values = replacement
    bag.values = make_pair()
    bag.values = make_fallible_pair()!
    bag.values = other.values
    return bag.values[0]
}

func make_pair(): [i32; 2] {
    return [9, 10]
}

func make_fallible_pair(): [i32; 2]! {
    return [11, 12]
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_fixed_array_optional_otherwise_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Bag {
    tag: i32
    values: [i32; 3]
}

func main(): i32 {
    var values: [i32; 3] = [0, 0, 0]
    let fallback: [i32; 3] = [1, 2, 3]
    var bag = Bag{ tag: 5, values: [0, 0, 0] }
    values = maybe_values(false) otherwise { [1, 2, 3] }
    values = maybe_values(false) otherwise { fallback }
    bag.values = maybe_values(true) otherwise { [90, 91, 92] }
    let field_success_total: i32 = sum(bag.values)
    bag.values = maybe_values(false) otherwise { make_values() }
    return sum(values) + field_success_total + sum(bag.values) + bag.tag
}

func maybe_values(flag: bool): [i32; 3]? {
    if flag {
        return [7, 8, 9]
    }
    return none
}

func make_values(): [i32; 3] {
    return [10, 11, 15]
}

func sum(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_aggregate_optional_otherwise_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    var header = Header{ tag: 0, ok: false, code: 0, len: 0 }
    let fallback = Triple{ first: 2, second: 8, third: 1, fourth: 1, fifth: 4 }
    var packet = Packet{
        prefix: 5,
        header: Header{ tag: 3, ok: false, code: 3, len: 3 },
        triple: Triple{ first: 1, second: 1, third: 1, fourth: 1, fifth: 1 },
    }
    header = maybe_header(false) otherwise { Header{ tag: 1, ok: false, code: 7, len: 2 } }
    packet.header = maybe_header(true) otherwise { Header{ tag: 9, ok: false, code: 90, len: 9 } }
    packet.triple = maybe_triple(false) otherwise { fallback }
    let returned = assign_with_return_fallback()
    return header_score(header) + header_score(packet.header) + triple_score(packet.triple) + returned + packet.prefix
}

func assign_with_return_fallback(): i32 {
    var header = Header{ tag: 0, ok: false, code: 0, len: 0 }
    header = maybe_header(false) otherwise { return 19 }
    return header.code
}

func header_score(header: Header): i32 {
    return header.code
}

func triple_score(triple: Triple): i32 {
    return triple.second + triple.fifth
}

func maybe_header(flag: bool): Header? {
    if flag {
        return Header{ tag: 4, ok: true, code: 10, len: 4 }
    }
    return none
}

func maybe_triple(flag: bool): Triple? {
    if flag {
        return Triple{ first: 3, second: 30, third: 3, fourth: 3, fifth: 3 }
    }
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_aggregate_optional_otherwise_member_root_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    let fallback = Packet{
        prefix: 5,
        header: Header{ tag: 1, ok: false, code: 7, len: 2 },
        triple: Triple{ first: 2, second: 8, third: 1, fourth: 1, fifth: 4 },
    }
    let code = (maybe_packet(false) otherwise { fallback }).header.code
    let triple = (maybe_packet(true) otherwise { fallback }).triple
    return code + triple.second + member_return_fallback()
}

func member_return_fallback(): i32 {
    let code = (maybe_packet(false) otherwise { return 11 }).header.code
    return code
}

func maybe_packet(flag: bool): Packet? {
    if flag {
        return Packet{
            prefix: 6,
            header: Header{ tag: 4, ok: true, code: 10, len: 4 },
            triple: Triple{ first: 3, second: 30, third: 3, fourth: 3, fifth: 3 },
        }
    }
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_generic_fixed_array_aggregate_field_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Box<T> {
    values: [T; 2]
}

func main(): i32 {
    var box = Box<i32>{ values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Box<i32>{ values: [20, 22] }
    box.values = [5, 6]
    box.values = replacement
    box.values = make_pair()
    box.values = other.values
    return box.values[0] + box.values[1]
}

func make_pair(): [i32; 2] {
    return [7, 8]
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_reachable_fixed_array_aggregate_field_control_assignment_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"copy struct Bag {
    values: [i32; 2]
}

func main(): i32 {
    var bag = Bag{ values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Bag{ values: [5, 6] }
    bag.values = if true {
        replacement
    } else {
        other.values
    }
    return bag.values[0]
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0435"
                    && diagnostic.message
                        == "Nocter v0 build cannot lower fixed array assignments outside supported replacement values yet"
            }),
            "{diagnostics:?}"
        );
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
    fn reports_terminal_if_inside_catch_block_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return source() catch error {
        if true {
            return 1
        } else {
            return 2
        }
    }
}

func source(): i32! {
    return 1
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower `catch` blocks outside the v0 runtime subset yet"
        );
    }

    #[test]
    fn reports_nested_otherwise_value_expression_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    return use_value((source() otherwise { 1 }) + 2)
}

func use_value(value: i32): i32 {
    return value
}

func source(): i32? {
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower `otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions yet"
        );
    }

    #[test]
    fn accepts_reachable_scalar_otherwise_direct_value_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct State {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
}

func main(): i32 {
    let state = State{
        count: maybe_i32(false) otherwise { 2 },
        byte: maybe_u8(true) otherwise { 1 },
        size: maybe_usize(false) otherwise { 9 },
        ok: maybe_bool(true) otherwise { false },
        text: maybe_text(false) otherwise { "Nocter" },
    }
    let branch = if false {
        maybe_i32(true) otherwise { 1 }
    } else {
        maybe_i32(false) otherwise { 4 }
    }
    return combine(
        maybe_i32(true) otherwise { 1 },
        maybe_u8(false) otherwise { 3 },
        maybe_usize(true) otherwise { 1 },
        maybe_bool(false) otherwise { true },
        maybe_text(true) otherwise { "bad" },
    ) + state.count + state.byte as i32 + branch
}

func combine(count: i32, byte: u8, size: usize, ok: bool, text: &str): i32 {
    if ok && size == 8 && text.len() == 4 {
        return count + byte as i32
    }
    return 0
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 8 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_reachable_scalar_otherwise_assignment_boundary() {
        let (sources, analysis) = analyze_text(
            r#"copy struct State {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
}

func main(): i32 {
    var count: i32 = 0
    var byte: u8 = 0
    var size: usize = 0
    var ok: bool = false
    var text: &str = "bad"
    var state = State{ count: 0, byte: 0, size: 0, ok: false, text: "bad" }
    count = maybe_i32(true) otherwise { 1 }
    byte = maybe_u8(false) otherwise { 12 }
    size = maybe_usize(true) otherwise { 1 }
    ok = maybe_bool(false) otherwise { true }
    text = maybe_text(false) otherwise { "Nocter" }
    state.count = maybe_i32(false) otherwise { 5 }
    state.byte = maybe_u8(true) otherwise { 1 }
    state.size = maybe_usize(false) otherwise { 8 }
    state.ok = maybe_bool(true) otherwise { false }
    state.text = maybe_text(true) otherwise { "lang" }
    let returned = assign_with_return_fallback()
    return count + byte as i32 + state.count + state.byte as i32 + returned
}

func assign_with_return_fallback(): i32 {
    var value: i32 = 0
    value = maybe_i32(false) otherwise { return 7 }
    return value
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 20 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_terminal_if_inside_otherwise_binding_before_ir_lowering() {
        let (sources, analysis) = analyze_text(
            r#"func main(): i32 {
    let value = source() otherwise {
        if true {
            return 1
        } else {
            return 2
        }
    }
    return value
}

func source(): i32? {
    return none
}
"#,
        );

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0435");
        assert_eq!(
            diagnostics[0].message,
            "Nocter v0 build cannot lower `otherwise` fallback blocks outside the v0 binding subset yet"
        );
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
