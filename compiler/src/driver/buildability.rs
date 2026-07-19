use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, Block, CallExpr, DropDecl, Expr, ForRangeStmt,
    FunctionDecl, ImplDecl, ImplMember, Item, Stmt, TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::CallTarget;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind, drop_function_name};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypecheckFacts;
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) fn v0_buildability_diagnostics(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    entry_name: &str,
) -> Vec<Diagnostic> {
    let Some(root) = analysis.root_file() else {
        return Vec::new();
    };

    let root_source = root.ast.span.source;
    let index = CallableIndex::new(analysis, root_source);
    let mut queue = VecDeque::from([CallTarget::same_file(entry_name)]);
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

        for file in &analysis.files {
            for item in &file.ast.items {
                match item {
                    Item::Function(function) => {
                        let target = call_target_for_source(
                            file.ast.span.source,
                            root_source,
                            function.name.clone(),
                        );
                        names.insert(function.name_span, function.name.clone());
                        definitions.insert(target, IndexedCallable::new_function(function, file));
                    }
                    Item::Impl(impl_) if impl_.interface_ty.is_none() => {
                        let Some(type_name) = impl_target_type_name(&impl_.target_ty) else {
                            continue;
                        };
                        for member in &impl_.members {
                            match member {
                                ImplMember::Method(method) => {
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
                                        IndexedCallable::new_method(impl_, body, file),
                                    );
                                }
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
        issues.extend(generic_function_issue(function));
        issues.extend(nested_fallible_return_issue(function, &file.resolved));

        Self {
            body: &function.body,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_method(impl_: &'a ImplDecl, body: &'a Block, file: &'a FileAnalysis) -> Self {
        Self {
            body,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues: generic_impl_issue(impl_).into_iter().collect(),
        }
    }

    fn new_drop(drop_: &'a DropDecl, impl_: &'a ImplDecl, file: &'a FileAnalysis) -> Self {
        Self {
            body: &drop_.body,
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
        root_source,
        names,
        queue,
        diagnostics,
    );
}

fn collect_block_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        );
    }
}

fn collect_statement_diagnostics(
    statement: &Stmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
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
                    root_source,
                    names,
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
                root_source,
                names,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
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
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.value,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Stmt::If(statement) => {
            push_explicit_move_condition_diagnostic(sources, &statement.condition, diagnostics);
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::IfIs(statement) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                statement.pattern_span,
                "`if is` pattern branches",
                "use the current scalar `if` subset, or keep this pattern code on the `check` path",
            ));
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::IfLet(statement) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                statement.span,
                "`if let` optional branches",
                "use `let ... else` in the current buildable optional subset",
            ));
            collect_expression_diagnostics(
                &statement.initializer,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Switch(statement) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                statement.span,
                "`match` statements",
                "use supported scalar control flow until enum match lowering is promoted",
            ));
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            for arm in &statement.arms {
                collect_block_diagnostics(
                    &arm.body,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
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
                    root_source,
                    names,
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
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.end,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Stmt::While(statement) => {
            push_explicit_move_condition_diagnostic(sources, &statement.condition, diagnostics);
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Stmt::WhileLet(statement) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                statement.span,
                "`while let` optional loops",
                "use the current `while` subset with explicit scalar state until `while let` lowering is promoted",
            ));
            collect_expression_diagnostics(
                &statement.initializer,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
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
                root_source,
                names,
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
                root_source,
                names,
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
        "call a void, never, or discardable scalar/view function, handle a discardable scalar/view fallible call with `?`, `!`, or `catch`, or bind/return the value explicitly",
    ))
}

fn expression_statement_is_supported(expression: &Expr, resolved: &ResolveOutput) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => match call_return_shape(call, resolved) {
            Some(
                ReturnShape::Void
                | ReturnShape::Never
                | ReturnShape::DiscardableScalar
                | ReturnShape::DiscardableView,
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
        _ => false,
    }
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
                | ReturnShape::DiscardableView => ReturnShape::FallibleDiscardable,
                ReturnShape::Never | ReturnShape::FallibleDiscardable | ReturnShape::Other => {
                    ReturnShape::Other
                }
            }
        }
        _ => ReturnShape::Other,
    }
}

fn push_explicit_move_condition_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(span) = first_explicit_move_span(condition) else {
        return;
    };

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "keep conditions to scalar/view values and non-moving calls until condition move lowering is promoted",
    ));
}

fn first_explicit_move_span(expression: &Expr) -> Option<ByteSpan> {
    match expression {
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
        Expr::InterpolatedString(expression) => expression.parts.iter().find_map(|part| {
            if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                first_explicit_move_span(&part.expression)
            } else {
                None
            }
        }),
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(first_explicit_move_span),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| first_explicit_move_span(&field.value)),
        Expr::Propagate(expression) => first_explicit_move_span(&expression.expression),
        Expr::Force(expression) => first_explicit_move_span(&expression.expression),
        Expr::Catch(expression) => first_explicit_move_span(&expression.expression),
        Expr::Borrow(expression) => first_explicit_move_span(&expression.expression),
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            Some(expression.operator_span)
        }
        Expr::Unary(expression) => first_explicit_move_span(&expression.operand),
        Expr::Binary(expression) => first_explicit_move_span(&expression.left)
            .or_else(|| first_explicit_move_span(&expression.right)),
        Expr::TypeConversion(expression) => first_explicit_move_span(&expression.expression),
        Expr::Call(expression) => first_explicit_move_span(&expression.callee).or_else(|| {
            expression
                .arguments
                .iter()
                .find_map(first_explicit_move_span)
        }),
        Expr::Member(expression) => first_explicit_move_span(&expression.object),
        Expr::Index(expression) => first_explicit_move_span(&expression.object)
            .or_else(|| first_explicit_move_span(&expression.index)),
        Expr::Group(expression) => first_explicit_move_span(&expression.expression),
        Expr::OptionalDefault(expression) => first_explicit_move_span(&expression.value)
            .or_else(|| first_explicit_move_span(&expression.default)),
        Expr::PatternConditional(expression) => first_explicit_move_span(&expression.target)
            .or_else(|| {
                expression
                    .arms
                    .iter()
                    .find_map(|arm| first_explicit_move_span(&arm.expression))
            })
            .or_else(|| first_explicit_move_span(&expression.fallback)),
    }
}

fn collect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
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
                        root_source,
                        names,
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
                    root_source,
                    names,
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
                    root_source,
                    names,
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
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Force(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Catch(expression) => {
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.catch_block,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Unary(expression) => collect_expression_diagnostics(
            &expression.operand,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Binary(expression) => {
            collect_expression_diagnostics(
                &expression.left,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.right,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Expr::TypeConversion(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Call(expression) => {
            if let Some(diagnostic) =
                unsupported_unloaded_imported_call_diagnostic(sources, expression, resolved)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_dynamic_failure_payload_diagnostic(
                sources,
                expression,
                resolved,
                root_source,
            ) {
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
            collect_expression_diagnostics(
                &expression.callee,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            if let Some(target) =
                call_target_for_call(expression, resolved, typecheck_facts, root_source, names)
            {
                queue.push_back(target);
            }
            for argument in &expression.arguments {
                collect_expression_diagnostics(
                    argument,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
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
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::Index(expression) => {
            collect_expression_diagnostics(
                &expression.object,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.index,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Expr::Group(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            root_source,
            names,
            queue,
            diagnostics,
        ),
        Expr::OptionalDefault(expression) => {
            collect_expression_diagnostics(
                &expression.value,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.default,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
        Expr::PatternConditional(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.question_span,
                "pattern conditional `?{}` expressions",
                "use supported scalar control flow until pattern conditional lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.target,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_expression_diagnostics(
                    &arm.expression,
                    sources,
                    resolved,
                    typecheck_facts,
                    root_source,
                    names,
                    queue,
                    diagnostics,
                );
            }
            collect_expression_diagnostics(
                &expression.fallback,
                sources,
                resolved,
                typecheck_facts,
                root_source,
                names,
                queue,
                diagnostics,
            );
        }
    }
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
                    if !borrow_argument_source_is_binding_or_field(&borrow.expression) =>
                {
                    Some(argument)
                }
                _ => None,
            }
        })?;

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "borrow call arguments from unsupported expressions",
        "borrow a local binding, an aggregate field rooted at a binding, or pass an existing borrow parameter until general borrow-place lowering is promoted",
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
    if !method_call_receiver_is_borrow(member.member_span, typecheck_facts) {
        return None;
    }
    if borrow_argument_source_is_binding_or_field(&member.object) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.object.span(),
        "method borrow receivers from unsupported expressions",
        "call the method on a local binding or an aggregate field rooted at a binding until general receiver-place lowering is promoted",
    ))
}

fn method_call_receiver_is_borrow(member_span: ByteSpan, typecheck_facts: &TypecheckFacts) -> bool {
    let Some((_span, label)) = typecheck_facts.call_hover_at_offset(member_span.start) else {
        return false;
    };
    let Some(receiver) = label
        .strip_prefix("method (")
        .and_then(|label| label.split_once(")."))
        .map(|(receiver, _)| receiver)
    else {
        return false;
    };
    receiver
        .split_once(": ")
        .is_some_and(|(_name, ty)| ty.starts_with('&'))
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

fn unsupported_dynamic_failure_payload_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    root_source: SourceId,
) -> Option<Diagnostic> {
    if !is_imported_error_constructor_call(call, resolved, root_source) {
        return None;
    }

    let argument = call
        .arguments
        .iter()
        .map(unwrap_group_expr)
        .find(|argument| matches!(argument, Expr::Call(_)))?;

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "dynamic failure payload arguments",
        "use string literals, an existing lowerable &str local, or error.code/error.message until general failure payload lowering is promoted",
    ))
}

fn is_imported_error_constructor_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    root_source: SourceId,
) -> bool {
    if let Some(symbol) = resolved.symbol_for_call(call)
        && symbol.declaration_span.source != root_source
        && let SymbolKind::Function(signature) | SymbolKind::Primitive(signature) = &symbol.kind
    {
        return signature_is_static_error_constructor(signature, resolved);
    }

    if let Some((_owner, function)) = resolved.associated_function_for_call(call)
        && function.name_span.source != root_source
    {
        return signature_is_static_error_constructor(&function.signature, resolved);
    }

    false
}

fn signature_is_static_error_constructor(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> bool {
    signature.parameters.len() == 2 && type_expr_resolves_to_error(&signature.return_type, resolved)
}

fn type_expr_resolves_to_error(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    let TypeExpr::Reference(reference) = ty else {
        return false;
    };

    if reference.name == "error" {
        return true;
    }

    resolved
        .type_symbol_by_name(&reference.name)
        .and_then(|symbol| symbol.alias_target.as_ref())
        .is_some_and(|target| type_expr_resolves_to_error(target, resolved))
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
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
) -> Option<CallTarget> {
    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(method_name_span) = typecheck_facts.method_call_target(member.member_span) {
            let target_name = names.get(&method_name_span)?.clone();
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

fn generic_function_issue(function: &FunctionDecl) -> Option<BuildabilityIssue> {
    if function.generics.parameters.is_empty() {
        return None;
    }

    Some(BuildabilityIssue {
        span: function.generics.span.unwrap_or(function.span),
        construct: "generic functions",
        help: "define a monomorphic wrapper until v0 monomorphization is promoted",
    })
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
    if impl_.generics.parameters.is_empty() && !type_expr_is_generic_instantiation(&impl_.target_ty)
    {
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

fn type_expr_is_generic_instantiation(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Generic(generic) if !generic.arguments.is_empty())
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
    use crate::analysis::{CompileUnit, analyze_compile_unit_with_entry};
    use crate::entry::DEFAULT_ENTRY_NAME;
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

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis, DEFAULT_ENTRY_NAME);

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

        let diagnostics = v0_buildability_diagnostics(&sources, &analysis, DEFAULT_ENTRY_NAME);

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
        let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new());
        let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
        let diagnostics = analysis.diagnostics();
        assert!(
            diagnostics.is_empty(),
            "unexpected frontend diagnostics: {diagnostics:?}"
        );

        (sources, analysis)
    }
}
