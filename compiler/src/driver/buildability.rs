use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AssignmentOperator, Block, DropDecl, Expr, FunctionDecl, ImplMember, Item, Stmt, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::ir::CallTarget;
use crate::resolve::{ResolveOutput, SymbolKind, drop_function_name};
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
                                    definitions
                                        .insert(target, IndexedCallable::new_method(body, file));
                                }
                                ImplMember::Drop(drop_) => {
                                    let name = drop_function_name(type_name);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    names.insert(drop_name_span(drop_.span), name.clone());
                                    definitions
                                        .insert(target, IndexedCallable::new_drop(drop_, file));
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
}

impl<'a> IndexedCallable<'a> {
    fn new_function(function: &'a FunctionDecl, file: &'a FileAnalysis) -> Self {
        Self {
            body: &function.body,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_method(body: &'a Block, file: &'a FileAnalysis) -> Self {
        Self {
            body,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_drop(drop_: &'a DropDecl, file: &'a FileAnalysis) -> Self {
        Self {
            body: &drop_.body,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
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
            if statement.operator != AssignmentOperator::Assign {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.operator_span,
                    "compound assignment statements",
                    "use `target = target op value` until compound assignment lowering is promoted",
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
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                statement.range_span,
                "range `for` loops",
                "use `while` with explicit scalar state until range `for` lowering is promoted",
            ));
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
