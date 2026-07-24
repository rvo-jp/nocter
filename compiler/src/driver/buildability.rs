use crate::abi::{AbiType, abi_value_from_type_expr};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis, call_specializations::collect_call_specializations,
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, Block, CallExpr, DropDecl, Expr, ForRangeStmt,
    FunctionDecl, IfLetStmt, ImplDecl, ImplMember, Item, Stmt, TypeExpr, WhileLetStmt,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::CallTarget;
use crate::resolve::{ResolveOutput, SymbolKind, TypeSymbolKind, drop_function_name};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts};
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

impl<'a> CallableIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        let mut names = HashMap::new();
        let call_specializations = collect_call_specializations(analysis);

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
                        definitions.insert(target, IndexedCallable::new_function(function, file));
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
                                        IndexedCallable::new_method(body, HashMap::new(), file),
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
                                        let target = call_target_for_source(
                                            file.ast.span.source,
                                            root_source,
                                            specialization.target_name.clone(),
                                        );
                                        definitions.insert(
                                            target,
                                            IndexedCallable::new_method(
                                                body,
                                                specialization.substitutions.clone(),
                                                file,
                                            ),
                                        );
                                    }
                                }
                                ImplMember::Method(_) => {}
                                ImplMember::Drop(drop_) => {
                                    let name = drop_function_name(type_name);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    names.insert(drop_name_span(drop_.span), name.clone());
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_drop(drop_, impl_, file),
                                    );
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
    fn new_function(function: &'a FunctionDecl, file: &'a FileAnalysis) -> Self {
        let mut issues = Vec::new();
        issues.extend(nested_fallible_return_issue(function, &file.resolved));

        Self {
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
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(nested_fallible_return_issue(function, &file.resolved));

        Self {
            body: &function.body,
            substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_method(
        body: &'a Block,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            body,
            substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues: Vec::new(),
        }
    }

    fn new_drop(drop_: &'a DropDecl, impl_: &'a ImplDecl, file: &'a FileAnalysis) -> Self {
        Self {
            body: &drop_.body,
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues: generic_impl_issue(impl_).into_iter().collect(),
        }
    }
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

    collect_block_diagnostics(
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

fn if_let_statement_is_buildable(statement: &IfLetStmt, resolved: &ResolveOutput) -> bool {
    let Expr::Call(call) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    matches!(
        optional_call_success_shape(call, resolved),
        Some(ReturnShape::DiscardableScalar | ReturnShape::DiscardableView)
    )
}

fn while_let_statement_is_buildable(statement: &WhileLetStmt, resolved: &ResolveOutput) -> bool {
    let Expr::Call(call) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    matches!(
        optional_call_success_shape(call, resolved),
        Some(ReturnShape::DiscardableScalar | ReturnShape::DiscardableView)
    )
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

fn pattern_conditional_expression_is_buildable(
    expression: &crate::ast::PatternConditionalExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    let Some(first_arm) = expression.arms.first() else {
        return false;
    };
    if expression.arms.iter().any(|arm| arm.payload.is_some()) {
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

    expression.arms.iter().all(|arm| {
        resolved
            .type_symbol_by_name(&arm.enum_name)
            .is_some_and(|symbol| symbol.canonical_name == target_symbol.canonical_name)
            && target_symbol
                .variants
                .iter()
                .any(|variant| variant.name == arm.variant_name)
            && pattern_conditional_value_expression_is_buildable(
                &arm.expression,
                resolved,
                typecheck_facts,
            )
    }) && pattern_conditional_value_expression_is_buildable(
        &expression.fallback,
        resolved,
        typecheck_facts,
    )
}

fn pattern_conditional_value_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::IntegerLiteral(_) | Expr::StringLiteral(_) | Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => resolved
            .local_symbol_for_identifier(identifier)
            .and_then(|symbol| typecheck_facts.binding_type_label(symbol.name_span))
            .is_some_and(type_label_is_scalar_or_view),
        Expr::Member(_) | Expr::Index(_) => true,
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved),
            Some(ReturnShape::DiscardableScalar | ReturnShape::DiscardableView)
        ),
        Expr::Propagate(propagation) => matches!(
            unwrap_group_expr(&propagation.expression),
            Expr::Call(call) if call_fallible_success_shape_is_scalar_or_view(call, resolved)
        ),
        Expr::Force(force) => matches!(
            unwrap_group_expr(&force.expression),
            Expr::Call(call) if call_fallible_success_shape_is_scalar_or_view(call, resolved)
        ),
        Expr::Catch(catch) => matches!(
            unwrap_group_expr(&catch.expression),
            Expr::Call(call) if call_fallible_success_shape_is_scalar_or_view(call, resolved)
        ),
        Expr::Binary(binary) => {
            pattern_conditional_value_expression_is_buildable(
                &binary.left,
                resolved,
                typecheck_facts,
            ) && pattern_conditional_value_expression_is_buildable(
                &binary.right,
                resolved,
                typecheck_facts,
            )
        }
        Expr::TypeConversion(conversion) => pattern_conditional_value_expression_is_buildable(
            &conversion.expression,
            resolved,
            typecheck_facts,
        ),
        Expr::PatternConditional(conditional) => {
            pattern_conditional_expression_is_buildable(conditional, resolved, typecheck_facts)
        }
        Expr::Group(_) => unreachable!("unwrap_group_expr removes groups"),
        Expr::InterpolatedString(_)
        | Expr::NoneLiteral(_)
        | Expr::ArrayLiteral(_)
        | Expr::StructLiteral(_)
        | Expr::Borrow(_)
        | Expr::Unary(_)
        | Expr::OptionalDefault(_) => false,
    }
}

fn call_fallible_success_shape_is_scalar_or_view(
    call: &CallExpr,
    resolved: &ResolveOutput,
) -> bool {
    let Some(signature) = resolved.call_signature_for_call(call) else {
        return false;
    };
    let TypeExpr::Fallible(fallible) = &signature.return_type else {
        return false;
    };
    matches!(
        return_shape_from_type_expr(&fallible.success, resolved),
        ReturnShape::DiscardableScalar | ReturnShape::DiscardableView
    )
}

fn type_label_is_scalar_or_view(label: &str) -> bool {
    matches!(
        label,
        "i32" | "u8" | "usize" | "bool" | "&str" | "&[u8]" | "&+[u8]"
    )
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
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
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
        Stmt::Binding(statement) => {
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
        Stmt::Assignment(statement) => {
            if !assignment_operator_is_buildable(statement, resolved, typecheck_facts) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.operator_span,
                    "compound assignment statements",
                    "use `i32` or `usize` whole-binding or aggregate-field compound assignment, or use `target = target op value` until broader compound assignment lowering is promoted",
                ));
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
        Stmt::IfLet(statement) => {
            if !if_let_statement_is_buildable(statement, resolved) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "`if let` optional branches",
                    "use a direct optional scalar/view call initializer, or use `let ... else` when the none path must exit immediately",
                ));
            }
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
        Stmt::WhileLet(statement) => {
            if !while_let_statement_is_buildable(statement, resolved) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "`while let` optional loops",
                    "use a direct optional scalar/view call in the current buildable `while let` subset",
                ));
            }
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

fn unsupported_expression_statement_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
) -> Option<Diagnostic> {
    if expression_statement_is_supported(expression, resolved) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span(),
        "value-producing expression statements",
        "call a void, never, or discardable scalar/view/aggregate function, handle a discardable scalar/view/aggregate fallible call with `?`, `!`, or `catch`, or bind/return the value explicitly",
    ))
}

fn expression_statement_is_supported(expression: &Expr, resolved: &ResolveOutput) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => match call_return_shape(call, resolved) {
            Some(
                ReturnShape::Void
                | ReturnShape::Never
                | ReturnShape::DiscardableScalar
                | ReturnShape::DiscardableView
                | ReturnShape::DiscardableAggregate,
            )
            | None => true,
            Some(ReturnShape::FallibleDiscardable | ReturnShape::Other) => false,
        },
        Expr::Propagate(expression) => {
            fallible_void_statement_inner_is_supported(&expression.expression, resolved)
        }
        Expr::Force(expression) => {
            fallible_void_statement_inner_is_supported(&expression.expression, resolved)
        }
        Expr::Catch(expression) => {
            fallible_void_statement_inner_is_supported(&expression.expression, resolved)
        }
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

fn fallible_void_statement_inner_is_supported(expression: &Expr, resolved: &ResolveOutput) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => match call_return_shape(call, resolved) {
            Some(ReturnShape::FallibleDiscardable) | None => true,
            Some(
                ReturnShape::Void
                | ReturnShape::Never
                | ReturnShape::DiscardableScalar
                | ReturnShape::DiscardableView
                | ReturnShape::DiscardableAggregate
                | ReturnShape::Other,
            ) => false,
        },
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
                Some("i32" | "usize")
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typecheck_facts)
        }
        _ => false,
    }
}

fn aggregate_field_compound_assignment_is_buildable(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    let Some((span, target)) = typecheck_facts.field_target_at_offset(member_span.start) else {
        return false;
    };
    if span != member_span {
        return false;
    }
    let Some(label) = typecheck_facts.declaration_hover_label(target) else {
        return false;
    };
    matches!(field_declaration_type_label(label), Some("i32" | "usize"))
}

fn field_declaration_type_label(label: &str) -> Option<&str> {
    let (_, ty) = label.rsplit_once(": ")?;
    Some(ty)
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

fn call_return_shape(call: &CallExpr, resolved: &ResolveOutput) -> Option<ReturnShape> {
    let signature = resolved.call_signature_for_call(call)?;
    Some(return_shape_from_type_expr(
        &signature.return_type,
        resolved,
    ))
}

fn optional_call_success_shape(call: &CallExpr, resolved: &ResolveOutput) -> Option<ReturnShape> {
    let signature = resolved.call_signature_for_call(call)?;
    Some(optional_success_shape_from_type_expr(
        &signature.return_type,
        resolved,
    ))
}

fn optional_success_shape_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> ReturnShape {
    optional_success_shape_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn optional_success_shape_from_type_expr_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> ReturnShape {
    match ty {
        TypeExpr::Optional(optional) => {
            return_shape_from_type_expr_inner(&optional.inner, resolved, resolving_names)
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
            let shape =
                optional_success_shape_from_type_expr_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            shape
        }
        _ => ReturnShape::Other,
    }
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
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            ReturnShape::DiscardableView
        }
        TypeExpr::Borrow(borrow)
            if matches!(
                borrow.inner.as_ref(),
                TypeExpr::View(view)
                    if matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
            ) =>
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
            if let Some(diagnostic) =
                unsupported_unloaded_imported_call_diagnostic(sources, expression, resolved)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) =
                unsupported_borrow_call_argument_diagnostic(sources, expression, resolved)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) =
                unsupported_method_borrow_receiver_diagnostic(sources, expression, typecheck_facts)
            {
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
            for argument in &expression.arguments {
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
        Expr::Member(expression) => collect_expression_diagnostics(
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
        ),
        Expr::Index(expression) => {
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
        Expr::OptionalDefault(expression) => {
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
            collect_expression_diagnostics(
                &expression.default,
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
        Expr::PatternConditional(expression) => {
            if !pattern_conditional_expression_is_buildable(expression, resolved, typecheck_facts) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    expression.question_span,
                    "pattern conditional `?{}` expressions",
                    "use payloadless enum `?{}` arms that produce scalar/view values, or keep payload pattern code on the `check` path",
                ));
            }
            collect_expression_diagnostics(
                &expression.target,
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
                collect_expression_diagnostics(
                    &arm.expression,
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
            collect_expression_diagnostics(
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

    let declaration_name = sources
        .get(symbol.declaration_span.source)?
        .text()
        .get(symbol.declaration_span.start..symbol.declaration_span.end)?;
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

fn source_is_std_process(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
) -> bool {
    let Some(nocter_home) = nocter_home else {
        return false;
    };

    sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .and_then(|path| path.strip_prefix(nocter_home).ok())
        .is_some_and(|relative| relative == Path::new("std/process.nct"))
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
) -> Option<Diagnostic> {
    let signature = resolved.call_signature_for_call(call)?;
    let argument = call
        .arguments
        .iter()
        .zip(signature.parameters.iter())
        .find_map(|(argument, parameter)| {
            if !type_expr_resolves_to_borrow(&parameter.ty, resolved) {
                return None;
            }
            match unwrap_group_expr(argument) {
                Expr::Borrow(borrow)
                    if borrow.is_readwrite
                        && !borrow_argument_source_is_binding_or_field(&borrow.expression) =>
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
        "borrow a mutable local binding or mutable aggregate field rooted at a binding until read-write temporary borrow lowering is promoted",
    ))
}

fn unsupported_method_borrow_receiver_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typecheck_facts.method_call_target(member.member_span)?;
    if !method_call_receiver_is_readwrite_borrow(member.member_span, typecheck_facts) {
        return None;
    }
    if borrow_argument_source_is_binding_or_field(&member.object) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.object.span(),
        "read-write method borrow receivers from unsupported expressions",
        "call the method on a mutable local binding or mutable aggregate field rooted at a binding until read-write temporary receiver lowering is promoted",
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

fn borrow_argument_source_is_binding_or_field(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
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

fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

fn nested_fallible_return_issue(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> Option<BuildabilityIssue> {
    if type_expr_fallible_depth(&function.return_type, resolved) <= 1 {
        return None;
    }

    Some(BuildabilityIssue {
        span: function.return_type.span(),
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

fn generic_impl_issue(impl_: &ImplDecl) -> Option<BuildabilityIssue> {
    if impl_.generics.parameters.is_empty() {
        return None;
    }

    Some(BuildabilityIssue {
        span: impl_
            .generics
            .span
            .unwrap_or_else(|| impl_.target_ty.span()),
        construct: "generic impl members",
        help: "use a non-generic impl target until v0 monomorphization is promoted",
    })
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
    fn does_not_report_reachable_payloadless_pattern_conditional() {
        let (sources, analysis) = analyze_text(
            r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    return choice ?{
        Choice.yes : 0
        : 1
    }
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
        let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), None);
        let analysis = analyze_executable_compile_unit(&sources, &unit);
        let diagnostics = analysis.diagnostics();
        assert!(
            diagnostics.is_empty(),
            "unexpected frontend diagnostics: {diagnostics:?}"
        );

        (sources, analysis)
    }
}
