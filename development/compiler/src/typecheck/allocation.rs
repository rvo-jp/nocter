//! Allocation capability validation and callable allocation-effect analysis.

use super::TypecheckSource;
use super::bindings::continuing_binding_type;
use super::calls::resolved_call_signature;
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_literal, environment_for_literal_pack_binding,
    environment_for_method, environment_for_switch_arm,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::provenance::{CallableId, CallableProvenanceSummaries};
use super::regions::region_binding_type;
use crate::ast::{Block, Expr, ImplMember, InterpolatedStringPart, Item, Stmt};
use crate::resolve::ResolveOutput;
use crate::semantics::{AllocationSource, AllocatorCapabilityKind, TrustedDeclarationRole};

pub(super) fn allocator_capability_kind(
    ty: &Type,
    resolved: &ResolveOutput,
) -> Option<AllocatorCapabilityKind> {
    let name = ty.nominal_name()?;
    let (symbol, _) = resolved.type_symbol_definition_by_reference_name(name)?;
    match resolved
        .trusted_declarations
        .role(symbol.declaration_span)?
    {
        TrustedDeclarationRole::AllocatorCapability(kind) => Some(kind),
        TrustedDeclarationRole::CurrentAllocationContext
        | TrustedDeclarationRole::AllocationOperation { .. }
        | TrustedDeclarationRole::RegionEnter
        | TrustedDeclarationRole::RegionRelease
        | TrustedDeclarationRole::AllocationAbort
        | TrustedDeclarationRole::IndependentFallibleError => None,
    }
}

pub(super) fn type_is_aborting_allocator_capability(ty: &Type, resolved: &ResolveOutput) -> bool {
    allocator_capability_kind(ty, resolved) == Some(AllocatorCapabilityKind::Aborting)
}

pub(super) fn infer_callable_allocation_effects(
    sources: &[TypecheckSource<'_>],
    summaries: &mut CallableProvenanceSummaries,
) {
    let callable_count = sources
        .iter()
        .map(|source| {
            source
                .ast
                .items
                .iter()
                .map(|item| {
                    match item {
                    Item::Function(_) => 1,
                    Item::Impl(impl_) => impl_
                        .members
                        .iter()
                        .filter(|member| {
                            matches!(member, ImplMember::Method(method) if method.body.is_some())
                        })
                        .count(),
                    _ => 0,
                }
                })
                .sum::<usize>()
        })
        .sum::<usize>();

    for _ in 0..=callable_count {
        let mut changed = false;
        for source in sources {
            for item in &source.ast.items {
                match item {
                    Item::Function(function) => {
                        let callable = CallableId::declared_at(if function.owner.is_some() {
                            function.member_name_span
                        } else {
                            function.name_span
                        });
                        if !summaries.needs_current_allocation_context(callable)
                            && block_needs_current_allocation_context(
                                &function.body,
                                source.resolved,
                                &mut environment_for_function(function, source.resolved),
                                summaries,
                            )
                        {
                            summaries.set_needs_current_allocation_context(callable);
                            changed = true;
                        }
                    }
                    Item::Impl(impl_) => {
                        for member in &impl_.members {
                            let ImplMember::Method(method) = member else {
                                continue;
                            };
                            let Some(body) = &method.body else {
                                continue;
                            };
                            let callable = CallableId::declared_at(method.name_span);
                            if !summaries.needs_current_allocation_context(callable)
                                && block_needs_current_allocation_context(
                                    body,
                                    source.resolved,
                                    &mut environment_for_method(method, source.resolved, impl_),
                                    summaries,
                                )
                            {
                                summaries.set_needs_current_allocation_context(callable);
                                changed = true;
                            }
                        }
                    }
                    Item::Literal(literal) => {
                        let callable = CallableId::declared_at(literal.span);
                        if !summaries.needs_current_allocation_context(callable)
                            && block_needs_current_allocation_context(
                                &literal.body,
                                source.resolved,
                                &mut environment_for_literal(literal, source.resolved),
                                summaries,
                            )
                        {
                            summaries.set_needs_current_allocation_context(callable);
                            changed = true;
                        }
                    }
                    Item::Import(_)
                    | Item::FromImport(_)
                    | Item::Primitive(_)
                    | Item::TypeAlias(_)
                    | Item::Struct(_)
                    | Item::Enum(_)
                    | Item::Interface(_) => {}
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn block_needs_current_allocation_context(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> bool {
    for statement in &block.statements {
        if statement_needs_current_allocation_context(statement, resolved, environment, summaries) {
            return true;
        }
    }
    block.result.as_ref().is_some_and(|result| {
        expression_needs_current_allocation_context(result, resolved, environment, summaries)
    })
}

fn statement_needs_current_allocation_context(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> bool {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
        Stmt::Return(statement) => statement.expression.as_ref().is_some_and(|expression| {
            expression_needs_current_allocation_context(
                expression,
                resolved,
                environment,
                summaries,
            )
        }),
        Stmt::Binding(statement) => {
            let needs = expression_needs_current_allocation_context(
                &statement.initializer,
                resolved,
                environment,
                summaries,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type,
                binding_kind_is_mutable(statement.kind),
            );
            needs
        }
        Stmt::Assignment(statement) => {
            expression_needs_current_allocation_context(
                &statement.target,
                resolved,
                environment,
                summaries,
            ) || expression_needs_current_allocation_context(
                &statement.value,
                resolved,
                environment,
                summaries,
            )
        }
        Stmt::If(statement) => {
            expression_needs_current_allocation_context(
                &statement.condition,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &statement.then_block,
                resolved,
                &mut environment.clone(),
                summaries,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_needs_current_allocation_context(
                    block,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Stmt::IfIs(statement) => {
            expression_needs_current_allocation_context(
                &statement.expression,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &statement.then_block,
                resolved,
                &mut environment_for_if_is_binding(statement, resolved, environment),
                summaries,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_needs_current_allocation_context(
                    block,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Stmt::Switch(statement) => {
            expression_needs_current_allocation_context(
                &statement.expression,
                resolved,
                environment,
                summaries,
            ) || statement.arms.iter().any(|arm| {
                block_needs_current_allocation_context(
                    &arm.body,
                    resolved,
                    &mut environment_for_switch_arm(
                        arm,
                        &statement.expression,
                        resolved,
                        environment,
                    ),
                    summaries,
                )
            }) || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                block_needs_current_allocation_context(
                    &arm.body,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Stmt::ForRange(statement) => {
            expression_needs_current_allocation_context(
                &statement.start,
                resolved,
                environment,
                summaries,
            ) || expression_needs_current_allocation_context(
                &statement.end,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &statement.body,
                resolved,
                &mut environment_for_for_range_binding(statement, resolved, environment),
                summaries,
            )
        }
        Stmt::LiteralPackFor(statement) => block_needs_current_allocation_context(
            &statement.body,
            resolved,
            &mut environment_for_literal_pack_binding(statement, environment),
            summaries,
        ),
        Stmt::While(statement) => {
            expression_needs_current_allocation_context(
                &statement.condition,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &statement.body,
                resolved,
                &mut environment.clone(),
                summaries,
            )
        }
        Stmt::Loop(statement) => block_needs_current_allocation_context(
            &statement.body,
            resolved,
            &mut environment.clone(),
            summaries,
        ),
        Stmt::Region(statement) => {
            let needs = expression_needs_current_allocation_context(
                &statement.allocator,
                resolved,
                environment,
                summaries,
            );
            let mut body_environment = environment.clone();
            body_environment.define(
                statement.name.clone(),
                region_binding_type(statement, resolved, environment),
            );
            needs
                || block_needs_current_allocation_context(
                    &statement.body,
                    resolved,
                    &mut body_environment,
                    summaries,
                )
        }
        Stmt::Expression(statement) => expression_needs_current_allocation_context(
            &statement.expression,
            resolved,
            environment,
            summaries,
        ),
    }
}

fn expression_needs_current_allocation_context(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> bool {
    match expression {
        Expr::TypedSequenceLiteral(literal) => {
            literal.using.is_none()
                || literal.elements.iter().any(|element| {
                    expression_needs_current_allocation_context(
                        element,
                        resolved,
                        environment,
                        summaries,
                    )
                })
                || literal.using.as_ref().is_some_and(|using| {
                    expression_needs_current_allocation_context(
                        &using.allocator,
                        resolved,
                        environment,
                        summaries,
                    )
                })
        }
        Expr::TypedStringLiteral(literal) => {
            literal.using.is_none()
                || literal.using.as_ref().is_some_and(|using| {
                    expression_needs_current_allocation_context(
                        &using.allocator,
                        resolved,
                        environment,
                        summaries,
                    )
                })
        }
        Expr::Call(call) => {
            let call_needs = resolved_call_signature(resolved, call, environment)
                .and_then(|signature| signature.declaration_span)
                .is_some_and(|declaration| {
                    summaries.needs_current_allocation_context(CallableId::declared_at(declaration))
                        || trusted_call_needs_current_context(resolved, declaration)
                });
            call_needs
                || expression_needs_current_allocation_context(
                    &call.callee,
                    resolved,
                    environment,
                    summaries,
                )
                || call.arguments.iter().any(|argument| {
                    expression_needs_current_allocation_context(
                        argument,
                        resolved,
                        environment,
                        summaries,
                    )
                })
        }
        Expr::InterpolatedString(expression) => expression.parts.iter().any(|part| match part {
            InterpolatedStringPart::Text(_) => false,
            InterpolatedStringPart::Expression(part) => {
                expression_needs_current_allocation_context(
                    &part.expression,
                    resolved,
                    environment,
                    summaries,
                )
            }
        }),
        Expr::ArrayLiteral(expression) => expression.elements.iter().any(|element| {
            expression_needs_current_allocation_context(element, resolved, environment, summaries)
        }),
        Expr::StructLiteral(expression) => expression.fields.iter().any(|field| {
            expression_needs_current_allocation_context(
                &field.value,
                resolved,
                environment,
                summaries,
            )
        }),
        Expr::Catch(expression) => {
            expression_needs_current_allocation_context(
                &expression.expression,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &expression.catch_block,
                resolved,
                &mut environment_for_catch(
                    expression.error_name.clone(),
                    &expression.expression,
                    resolved,
                    environment,
                ),
                summaries,
            )
        }
        Expr::Otherwise(expression) => {
            expression_needs_current_allocation_context(
                &expression.value,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &expression.fallback,
                resolved,
                &mut environment.clone(),
                summaries,
            )
        }
        Expr::If(expression) => {
            expression_needs_current_allocation_context(
                &expression.condition,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &expression.then_block,
                resolved,
                &mut environment.clone(),
                summaries,
            ) || expression.else_block.as_ref().is_some_and(|block| {
                block_needs_current_allocation_context(
                    block,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Expr::IfIs(expression) => {
            expression_needs_current_allocation_context(
                &expression.expression,
                resolved,
                environment,
                summaries,
            ) || block_needs_current_allocation_context(
                &expression.then_block,
                resolved,
                &mut environment_for_if_is_binding(expression, resolved, environment),
                summaries,
            ) || expression.else_block.as_ref().is_some_and(|block| {
                block_needs_current_allocation_context(
                    block,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Expr::Match(expression) => {
            expression_needs_current_allocation_context(
                &expression.expression,
                resolved,
                environment,
                summaries,
            ) || expression.arms.iter().any(|arm| {
                block_needs_current_allocation_context(
                    &arm.body,
                    resolved,
                    &mut environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        resolved,
                        environment,
                    ),
                    summaries,
                )
            }) || expression.wildcard_arm.as_ref().is_some_and(|arm| {
                block_needs_current_allocation_context(
                    &arm.body,
                    resolved,
                    &mut environment.clone(),
                    summaries,
                )
            })
        }
        Expr::Index(expression) => {
            expression_needs_current_allocation_context(
                &expression.object,
                resolved,
                environment,
                summaries,
            ) || expression_needs_current_allocation_context(
                &expression.index,
                resolved,
                environment,
                summaries,
            )
        }
        Expr::Binary(expression) => {
            expression_needs_current_allocation_context(
                &expression.left,
                resolved,
                environment,
                summaries,
            ) || expression_needs_current_allocation_context(
                &expression.right,
                resolved,
                environment,
                summaries,
            )
        }
        Expr::Propagate(expression) => expression_needs_current_allocation_context(
            &expression.expression,
            resolved,
            environment,
            summaries,
        ),
        Expr::Force(expression) => expression_needs_current_allocation_context(
            &expression.expression,
            resolved,
            environment,
            summaries,
        ),
        Expr::Borrow(expression) => expression_needs_current_allocation_context(
            &expression.expression,
            resolved,
            environment,
            summaries,
        ),
        Expr::Unary(expression) => expression_needs_current_allocation_context(
            &expression.operand,
            resolved,
            environment,
            summaries,
        ),
        Expr::TypeConversion(expression) => expression_needs_current_allocation_context(
            &expression.expression,
            resolved,
            environment,
            summaries,
        ),
        Expr::Member(expression) => expression_needs_current_allocation_context(
            &expression.object,
            resolved,
            environment,
            summaries,
        ),
        Expr::Group(expression) => expression_needs_current_allocation_context(
            &expression.expression,
            resolved,
            environment,
            summaries,
        ),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => false,
    }
}

fn trusted_call_needs_current_context(
    resolved: &ResolveOutput,
    declaration: crate::source::ByteSpan,
) -> bool {
    match resolved.trusted_declarations.role(declaration) {
        Some(TrustedDeclarationRole::CurrentAllocationContext) => true,
        Some(TrustedDeclarationRole::AllocationOperation {
            source: AllocationSource::CurrentContext,
            ..
        }) => true,
        Some(
            TrustedDeclarationRole::AllocatorCapability(_)
            | TrustedDeclarationRole::AllocationOperation {
                source: AllocationSource::Input(_),
                ..
            }
            | TrustedDeclarationRole::RegionEnter
            | TrustedDeclarationRole::RegionRelease
            | TrustedDeclarationRole::AllocationAbort
            | TrustedDeclarationRole::IndependentFallibleError,
        )
        | None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Item;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::semantics::TrustedDeclarationFacts;
    use crate::source::SourceMap;

    #[test]
    fn infers_current_allocation_context_through_call_graph() {
        let text = r#"copy struct Arena {
    id: usize
}

primitive current_allocator(): Arena

func direct(): Arena {
    return current_allocator()
}

func indirect(): Arena {
    return direct()
}

func main(): i32 {
    return 0
}
"#;
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ast = parsed.ast.unwrap();
        let mut resolved = resolve(&sources, &ast);
        let mut trusted = TrustedDeclarationFacts::default();
        let mut direct = None;
        let mut indirect = None;
        for item in &ast.items {
            match item {
                Item::Primitive(primitive) if primitive.name == "current_allocator" => trusted
                    .insert(
                        primitive.name_span,
                        TrustedDeclarationRole::CurrentAllocationContext,
                    ),
                Item::Function(function) if function.name == "direct" => {
                    direct = Some(CallableId::declared_at(function.name_span));
                }
                Item::Function(function) if function.name == "indirect" => {
                    indirect = Some(CallableId::declared_at(function.name_span));
                }
                _ => {}
            }
        }
        resolved.trusted_declarations = trusted;
        let mut summaries = CallableProvenanceSummaries::default();
        infer_callable_allocation_effects(&[TypecheckSource::new(&ast, &resolved)], &mut summaries);

        assert!(summaries.needs_current_allocation_context(direct.unwrap()));
        assert!(summaries.needs_current_allocation_context(indirect.unwrap()));
    }
}
