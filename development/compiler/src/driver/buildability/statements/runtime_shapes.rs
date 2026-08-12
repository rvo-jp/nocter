use super::*;

pub(in crate::driver::buildability) fn expression_statement_is_supported(
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

pub(in crate::driver::buildability) fn catch_fallback_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    (block.statements.is_empty() && block.result.is_none())
        || otherwise_binding_fallback_runtime_shape_is_buildable(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

pub(in crate::driver::buildability) fn otherwise_return_fallback_runtime_shape_is_buildable(
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
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Region(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

pub(in crate::driver::buildability) fn otherwise_binding_fallback_runtime_shape_is_buildable(
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
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Region(_)
        | Stmt::Drop(_) => false,
    }
}

pub(in crate::driver::buildability) fn otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
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
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Region(_)
        | Stmt::Return(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

pub(in crate::driver::buildability) fn otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
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

pub(in crate::driver::buildability) fn fallible_void_statement_inner_is_supported(
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
