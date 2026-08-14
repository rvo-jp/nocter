//! Authoritative coverage classification for the first scalar MIR route.
//! A body rejected here remains on the legacy route; once accepted, MIR
//! construction and validation errors are authoritative.

use super::SemanticInputs;
use super::expressions::{mir_assignment_operator, mir_binary_operator, mir_comparison_operator};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BindingStmt, Block, Expr, ForRangeStmt, IfStmt, LoopStmt,
    RegionStmt, Stmt, WhileStmt,
};
use crate::literals::decode_integer_literal_value;
use crate::mir::ComparisonOperator;
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::typecheck::{CheckedScalarType, PartialSemantic, TypedHir};

#[derive(Debug, Clone, Copy)]
pub(super) enum ScalarStatement<'a> {
    Binding(&'a BindingStmt),
    Assignment(&'a AssignmentStmt),
    If(&'a IfStmt),
    ForRange(&'a ForRangeStmt),
    Loop(&'a LoopStmt),
    While(&'a WhileStmt),
    Region(&'a RegionStmt),
    Expression(&'a Expr),
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ScalarTail<'a> {
    Expression(&'a Expr),
    Return(&'a Expr),
    Conditional(&'a IfStmt),
}

impl<'a> ScalarTail<'a> {
    pub(super) fn expression(self) -> Option<&'a Expr> {
        match self {
            Self::Expression(expression) | Self::Return(expression) => Some(expression),
            Self::Conditional(_) => None,
        }
    }

    pub(super) fn conditional(self) -> Option<&'a IfStmt> {
        match self {
            Self::Expression(Expr::If(if_)) | Self::Return(Expr::If(if_)) => Some(if_),
            Self::Conditional(if_) => Some(if_),
            Self::Expression(_) | Self::Return(_) => None,
        }
    }

    pub(super) fn is_explicit_return(self) -> bool {
        matches!(self, Self::Return(_))
    }

    pub(super) fn is_supported(self, semantic: SemanticInputs<'_>) -> bool {
        match self {
            Self::Expression(Expr::Call(call)) | Self::Return(Expr::Call(call)) => {
                scalar_tail_call_is_supported(
                    call,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                )
            }
            Self::Expression(Expr::If(if_)) | Self::Return(Expr::If(if_)) => {
                scalar_conditional_is_supported(
                    if_,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                )
            }
            Self::Expression(expression) | Self::Return(expression) => {
                scalar_expression_is_supported(
                    expression,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                )
            }
            Self::Conditional(if_) => scalar_conditional_is_supported(
                if_,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
        }
    }

    pub(super) fn result_type(self, typed_hir: &TypedHir) -> Option<crate::semantic::TyId> {
        match self {
            Self::Expression(expression) | Self::Return(expression) => {
                known_expression_type(expression, typed_hir)
            }
            Self::Conditional(if_) => scalar_value_block_result_type(&if_.then_block, typed_hir),
        }
    }
}

fn scalar_tail_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        // A failure-only `error` call can acquire the surrounding success
        // type contextually, but it is not a scalar-returning call. Until MIR
        // carries failure payload values explicitly, leave that construct on
        // the outcome-aware route instead of manufacturing a scalar result.
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
        && effective_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn scalar_value_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn aggregate_value_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && effective_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .is_some_and(|ty| {
                matches!(
                    crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                        resolved_sources.get(&source).copied()
                    })
                    .map(|value| value.ty),
                    Ok(crate::abi::AbiType::Struct(_))
                        | Ok(crate::abi::AbiType::Array { .. })
                        | Ok(crate::abi::AbiType::Enum(_))
                        | Ok(crate::abi::AbiType::Outcome { .. })
                )
            })
}

fn scalar_call_shape_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let callee_supported = if let Some(fact) = typed_hir.callable_call(call.span) {
        typed_hir
            .type_id(&fact.specialization.callable_ty)
            .is_some()
            && typed_hir.type_id(&fact.receiver_ty).is_some()
            && match fact.specialization.capability {
                crate::ast::CallableCapability::Readonly
                | crate::ast::CallableCapability::Readwrite => {
                    super::borrows::source_place_is_supported(
                        &call.callee,
                        SemanticInputs {
                            resolved,
                            resolved_sources,
                            typed_hir,
                        },
                    )
                }
                crate::ast::CallableCapability::Consuming => callable_owned_receiver_is_supported(
                    &call.callee,
                    SemanticInputs {
                        resolved,
                        resolved_sources,
                        typed_hir,
                    },
                ),
            }
    } else {
        match call.callee.without_groups() {
            Expr::Identifier(callee) => {
                typed_hir
                    .function_call_target(callee.span)
                    .and_then(|target| resolved.semantic_db.definition(target))
                    .is_some_and(|definition| {
                        definition.kind == crate::semantic::DefinitionKind::Function
                    })
                    && typed_hir
                        .function_call_specialization(call.span)
                        .is_none_or(|specialization| {
                            specialization
                                .ordered_type_arguments()
                                .is_some_and(|arguments| {
                                    arguments
                                        .into_iter()
                                        .all(|ty| typed_hir.type_id(ty).is_some())
                                })
                        })
            }
            Expr::Member(member) => {
                let Some(definition) = typed_hir.method_call_target(member.member_span) else {
                    return false;
                };
                if resolved.semantic_db.definition(definition).is_none() {
                    return false;
                }
                let specialization_supported = typed_hir
                    .method_call_specialization(member.member_span)
                    .is_none_or(|specialization| {
                        typed_hir.type_id(&specialization.self_ty).is_some()
                            && specialization
                                .ordered_type_arguments()
                                .is_some_and(|arguments| {
                                    arguments
                                        .into_iter()
                                        .all(|ty| typed_hir.type_id(ty).is_some())
                                })
                    });
                specialization_supported
                    && method_receiver_is_supported(
                        member,
                        SemanticInputs {
                            resolved,
                            resolved_sources,
                            typed_hir,
                        },
                    )
            }
            _ => false,
        }
    };
    callee_supported
        && call.arguments.iter().all(|argument| {
            let Some(ty) = known_expression_type(argument, typed_hir) else {
                return false;
            };
            let semantic = SemanticInputs {
                resolved,
                resolved_sources,
                typed_hir,
            };
            scalar_type(ty, typed_hir).is_some()
                && scalar_expression_is_supported(argument, resolved, resolved_sources, typed_hir)
                || matches!(
                    value_representation(ty, semantic),
                    Some(representation @ crate::mir::ValueRepresentation::View(_))
                        if value_expression_is_supported(argument, representation, semantic)
                )
                || aggregate_operand_is_supported(argument, resolved, resolved_sources, typed_hir)
                || super::aggregates::literal_is_supported(argument, semantic)
                || borrow_argument_is_supported(argument, semantic)
        })
}

fn callable_owned_receiver_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return false;
    };
    semantic
        .resolved
        .local_symbol_for_identifier(identifier)
        .and_then(|symbol| semantic.typed_hir.binding_type_expr(symbol.id))
        .is_some_and(|ty| {
            crate::typecheck::type_expr_is_copy(ty, semantic.resolved) == Some(true)
                || super::super::drop_plans::is_supported(
                    ty,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                )
        })
}

fn method_receiver_is_supported(
    member: &crate::ast::MemberExpr,
    semantic: SemanticInputs<'_>,
) -> bool {
    let Some(kind) = semantic
        .typed_hir
        .method_call_receiver_kind(member.member_span)
    else {
        return false;
    };
    if kind != crate::typecheck::TypecheckMethodReceiverKind::Owned {
        if semantic
            .typed_hir
            .method_call_receiver_type(member.member_span)
            .and_then(|ty| value_representation(ty, semantic))
            .is_some_and(|representation| {
                matches!(representation, crate::mir::ValueRepresentation::View(_))
                    && value_expression_is_supported(&member.object, representation, semantic)
            })
        {
            return true;
        }
        return super::borrows::source_place_is_supported(&member.object, semantic);
    }
    let Some(ty) = known_expression_type(&member.object, semantic.typed_hir) else {
        return false;
    };
    scalar_type(ty, semantic.typed_hir).is_some()
        && scalar_expression_is_supported(
            &member.object,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
        || value_representation(ty, semantic).is_some_and(|representation| {
            matches!(representation, crate::mir::ValueRepresentation::View(_))
                && value_expression_is_supported(&member.object, representation, semantic)
        })
        || aggregate_operand_is_supported(
            &member.object,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
}

pub(super) fn borrow_argument_is_supported(
    expression: &Expr,
    semantic: SemanticInputs<'_>,
) -> bool {
    borrow_identifier_is_supported(expression, semantic.resolved, semantic.typed_hir)
        || super::borrows::expression_is_supported(expression, semantic)
}

pub(super) fn borrow_identifier_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return false;
    };
    resolved
        .local_symbol_for_identifier(identifier)
        .and_then(|symbol| typed_hir.binding_type_expr(symbol.id))
        .is_some_and(|ty| matches!(ty, crate::ast::TypeExpr::Borrow(_)))
}

pub(super) fn aggregate_operand_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let (expression, explicitly_moved) = match expression.without_groups() {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            (unary.operand.without_groups(), true)
        }
        expression => (expression, false),
    };
    let ty = match expression {
        Expr::Identifier(identifier) => resolved
            .local_symbol_for_identifier(identifier)
            .and_then(|symbol| typed_hir.binding_type_expr(symbol.id)),
        Expr::Member(member)
            if super::projections::owned_field_is_supported(
                member,
                SemanticInputs {
                    resolved,
                    resolved_sources,
                    typed_hir,
                },
            ) =>
        {
            known_expression_type(expression, typed_hir)
                .and_then(|ty| typed_hir.type_expr_by_id(ty))
        }
        _ => None,
    };
    let Some(ty) = ty else { return false };
    let aggregate = matches!(
        crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
            resolved_sources.get(&source).copied()
        })
        .map(|value| value.ty),
        Ok(crate::abi::AbiType::Struct(_))
            | Ok(crate::abi::AbiType::Array { .. })
            | Ok(crate::abi::AbiType::Enum(_))
            | Ok(crate::abi::AbiType::Outcome { .. })
    );
    if !aggregate {
        return false;
    }
    match crate::typecheck::type_expr_is_copy(ty, resolved) {
        Some(true) => !explicitly_moved,
        Some(false) => {
            explicitly_moved
                && super::super::drop_plans::is_supported(ty, resolved, resolved_sources, typed_hir)
        }
        None => false,
    }
}

impl<'a> ScalarStatement<'a> {
    pub(super) fn is_supported(self, semantic: SemanticInputs<'_>) -> bool {
        self.is_supported_in_context(
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
            false,
        )
    }

    fn is_supported_in_context(
        self,
        resolved: &ResolveOutput,
        resolved_sources: &crate::resolve::ResolvedSources<'_>,
        typed_hir: &TypedHir,
        in_loop: bool,
    ) -> bool {
        match self {
            Self::Binding(binding) => {
                let scalar = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .is_some_and(|symbol| {
                        binding_scalar_type(symbol, typed_hir).is_some()
                            && typed_hir
                                .binding_type_expr(symbol)
                                .and_then(|ty| typed_hir.type_id(ty))
                                .is_some()
                    });
                let borrow = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .and_then(|symbol| typed_hir.binding_type_expr(symbol))
                    .is_some_and(|ty| matches!(ty, crate::ast::TypeExpr::Borrow(_)))
                    && borrow_expression_is_supported(
                        &binding.initializer,
                        SemanticInputs {
                            resolved,
                            resolved_sources,
                            typed_hir,
                        },
                    );
                let aggregate = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .and_then(|symbol| typed_hir.binding_type_expr(symbol))
                    .is_some_and(|ty| {
                        (crate::typecheck::type_expr_is_copy(ty, resolved) == Some(true)
                            || super::super::drop_plans::is_supported(
                                ty,
                                resolved,
                                resolved_sources,
                                typed_hir,
                            ))
                            && matches!(
                                crate::abi::abi_value_from_type_expr_with_resolver(
                                    ty,
                                    resolved,
                                    |source| resolved_sources.get(&source).copied(),
                                )
                                .map(|value| value.ty),
                                Ok(crate::abi::AbiType::Struct(_))
                                    | Ok(crate::abi::AbiType::Array { .. })
                                    | Ok(crate::abi::AbiType::Enum(_))
                                    | Ok(crate::abi::AbiType::Outcome { .. })
                            )
                    })
                    && match binding.initializer.without_groups() {
                        Expr::StructLiteral(_)
                        | Expr::ArrayLiteral(_)
                        | Expr::Member(_)
                        | Expr::Closure(_) => super::aggregates::literal_is_supported(
                            &binding.initializer,
                            SemanticInputs {
                                resolved,
                                resolved_sources,
                                typed_hir,
                            },
                        ),
                        Expr::Call(call) => {
                            super::aggregates::literal_is_supported(
                                &binding.initializer,
                                SemanticInputs {
                                    resolved,
                                    resolved_sources,
                                    typed_hir,
                                },
                            ) || aggregate_value_call_is_supported(
                                call,
                                resolved,
                                resolved_sources,
                                typed_hir,
                            )
                        }
                        Expr::Force(_) | Expr::Propagate(_) => value_expression_is_supported(
                            &binding.initializer,
                            crate::mir::ValueRepresentation::Aggregate,
                            SemanticInputs {
                                resolved,
                                resolved_sources,
                                typed_hir,
                            },
                        ),
                        _ => false,
                    };
                (scalar
                    && known_expression_type(&binding.initializer, typed_hir).is_some()
                    && scalar_expression_is_supported(
                        &binding.initializer,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    ))
                    || borrow
                    || aggregate
            }
            Self::Assignment(assignment) => {
                let target_is_supported = match &assignment.target {
                    Expr::Identifier(identifier) => resolved
                        .local_symbol_for_identifier(identifier)
                        .is_some_and(|symbol| binding_scalar_type(symbol.id, typed_hir).is_some()),
                    Expr::Index(index) => {
                        known_expression_type(&assignment.target, typed_hir)
                            .and_then(|ty| scalar_type(ty, typed_hir))
                            .is_some()
                            && super::indexes::is_supported(
                                index,
                                SemanticInputs {
                                    resolved,
                                    resolved_sources,
                                    typed_hir,
                                },
                            )
                    }
                    _ => false,
                };
                (assignment.operator == AssignmentOperator::Assign
                    || mir_assignment_operator(assignment.operator).is_some())
                    && target_is_supported
                    && known_expression_type(&assignment.value, typed_hir).is_some()
                    && scalar_expression_is_supported(
                        &assignment.value,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
            }
            Self::While(statement) => {
                scalar_expression_is_supported(
                    &statement.condition,
                    resolved,
                    resolved_sources,
                    typed_hir,
                ) && known_expression_type(&statement.condition, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(crate::mir::ScalarType::Bool)
                    && scalar_loop_block_statements(
                        &statement.body,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    .is_some()
            }
            Self::Region(statement) => {
                region_allocator_is_supported(statement, resolved, resolved_sources, typed_hir)
                    && scalar_linear_block_statements(
                        &statement.body,
                        resolved,
                        resolved_sources,
                        typed_hir,
                        in_loop,
                    )
                    .is_some()
            }
            Self::Expression(expression) => match expression.without_groups() {
                Expr::Call(call) => {
                    effect_call_is_supported(call, resolved, resolved_sources, typed_hir)
                }
                Expr::Force(force) => {
                    let Expr::Call(call) = force.expression.without_groups() else {
                        return false;
                    };
                    effect_outcome_call_is_supported(call, resolved, resolved_sources, typed_hir)
                }
                Expr::Propagate(propagate) => {
                    let Expr::Call(call) = propagate.expression.without_groups() else {
                        return false;
                    };
                    effect_outcome_call_is_supported(call, resolved, resolved_sources, typed_hir)
                }
                _ => false,
            },
            Self::If(statement) => {
                scalar_expression_is_supported(
                    &statement.condition,
                    resolved,
                    resolved_sources,
                    typed_hir,
                ) && known_expression_type(&statement.condition, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(crate::mir::ScalarType::Bool)
                    && scalar_conditional_statement_is_supported(
                        statement,
                        resolved,
                        resolved_sources,
                        typed_hir,
                        in_loop,
                    )
            }
            Self::ForRange(statement) => {
                let Some(symbol) = resolved.local_symbol_id_at_name_span(statement.name_span)
                else {
                    return false;
                };
                let Some(binding_scalar) = binding_scalar_type(symbol, typed_hir) else {
                    return false;
                };
                matches!(
                    binding_scalar,
                    crate::mir::ScalarType::I32 | crate::mir::ScalarType::Usize
                ) && known_expression_type(&statement.start, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(binding_scalar)
                    && known_expression_type(&statement.end, typed_hir)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        == Some(binding_scalar)
                    && scalar_expression_is_supported(
                        &statement.start,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    && scalar_expression_is_supported(
                        &statement.end,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    && scalar_loop_block_statements(
                        &statement.body,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    .is_some()
            }
            Self::Loop(statement) => {
                scalar_loop_block_statements(&statement.body, resolved, resolved_sources, typed_hir)
                    .is_some()
            }
            Self::Break | Self::Continue => in_loop,
        }
    }
}

pub(super) fn borrow_expression_is_supported(
    expression: &Expr,
    semantic: SemanticInputs<'_>,
) -> bool {
    super::borrows::expression_is_supported(expression, semantic)
}

fn scalar_statement(statement: &Stmt) -> Option<ScalarStatement<'_>> {
    match statement {
        Stmt::Binding(binding) => Some(ScalarStatement::Binding(binding)),
        Stmt::Assignment(assignment) => Some(ScalarStatement::Assignment(assignment)),
        Stmt::If(statement) => Some(ScalarStatement::If(statement)),
        Stmt::ForRange(statement) => Some(ScalarStatement::ForRange(statement)),
        Stmt::Loop(statement) => Some(ScalarStatement::Loop(statement)),
        Stmt::While(statement) => Some(ScalarStatement::While(statement)),
        Stmt::Region(statement) => Some(ScalarStatement::Region(statement)),
        Stmt::Expression(statement) => Some(ScalarStatement::Expression(&statement.expression)),
        Stmt::Break(_) => Some(ScalarStatement::Break),
        Stmt::Continue(_) => Some(ScalarStatement::Continue),
        _ => None,
    }
}

fn effect_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .is_some_and(|ty| {
                matches!(ty, crate::ast::TypeExpr::Reference(reference) if reference.name == "void")
            })
}

fn effect_outcome_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .is_some_and(|ty| {
                let shape = crate::outcomes::outcome_shape_with_resolver(ty, resolved, |source| {
                    resolved_sources.get(&source).copied()
                });
                shape.layers == [crate::outcomes::OutcomeLayer::Fallible]
                    && matches!(
                        shape.payload,
                        crate::ast::TypeExpr::Reference(reference) if reference.name == "void"
                    )
            })
}

fn region_allocator_is_supported(
    statement: &RegionStmt,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let Expr::Identifier(identifier) = statement.allocator.without_groups() else {
        return false;
    };
    if resolved.local_symbol_for_identifier(identifier).is_none() {
        return false;
    }
    let Some(symbol) = resolved.local_symbol_id_at_name_span(statement.name_span) else {
        return false;
    };
    let Some(ty) = typed_hir.binding_type_expr(symbol) else {
        return false;
    };
    let Ok(value) = crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    }) else {
        return false;
    };
    let crate::abi::AbiType::Struct(fields) = value.ty else {
        return false;
    };
    ["state", "kind"].into_iter().all(|name| {
        fields
            .iter()
            .any(|field| field.name == name && field.ty == crate::abi::AbiType::Usize)
    })
}

pub(super) fn scalar_linear_block_statements<'a>(
    block: &'a Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    in_loop: bool,
) -> Option<Vec<ScalarStatement<'a>>> {
    if block.result.is_some() {
        return None;
    }
    let statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .map(scalar_statement)
        .collect::<Option<Vec<_>>>()?;
    let mut exited = false;
    for statement in &statements {
        if exited
            || matches!(
                statement,
                ScalarStatement::If(_)
                    | ScalarStatement::While(_)
                    | ScalarStatement::ForRange(_)
                    | ScalarStatement::Loop(_)
            )
            || !statement.is_supported_in_context(resolved, resolved_sources, typed_hir, in_loop)
        {
            return None;
        }
        exited = matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        );
    }
    Some(statements)
}

fn scalar_conditional_statement_is_supported(
    statement: &IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    in_loop: bool,
) -> bool {
    let Some(then_statements) = scalar_linear_block_statements(
        &statement.then_block,
        resolved,
        resolved_sources,
        typed_hir,
        in_loop,
    ) else {
        return false;
    };
    let else_statements = statement
        .else_block
        .as_ref()
        .map(|block| {
            scalar_linear_block_statements(block, resolved, resolved_sources, typed_hir, in_loop)
        })
        .unwrap_or_else(|| Some(Vec::new()));
    let Some(else_statements) = else_statements else {
        return false;
    };
    let then_exits = then_statements.last().is_some_and(|statement| {
        matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        )
    });
    let else_exits = else_statements.last().is_some_and(|statement| {
        matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        )
    });
    !(then_exits && else_exits)
}

pub(super) fn scalar_loop_block_statements<'a>(
    block: &'a Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> Option<Vec<ScalarStatement<'a>>> {
    if block.result.is_some() {
        return None;
    }
    let statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .map(scalar_statement)
        .collect::<Option<Vec<_>>>()?;
    let mut exited = false;
    for statement in &statements {
        if exited
            || matches!(
                statement,
                ScalarStatement::While(_) | ScalarStatement::ForRange(_) | ScalarStatement::Loop(_)
            )
            || !statement.is_supported_in_context(resolved, resolved_sources, typed_hir, true)
        {
            return None;
        }
        exited = matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        );
    }
    Some(statements)
}

pub(super) fn scalar_body_parts(
    block: &Block,
) -> Option<(Vec<ScalarStatement<'_>>, ScalarTail<'_>)> {
    let runtime_statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .collect::<Vec<_>>();
    let (body_statements, tail) = if let Some(result) = block.result.as_deref() {
        (
            runtime_statements.as_slice(),
            ScalarTail::Expression(result),
        )
    } else {
        let (last, leading) = runtime_statements.split_last()?;
        let tail = match last {
            Stmt::Return(statement) => ScalarTail::Return(statement.expression.as_ref()?),
            Stmt::If(if_) => ScalarTail::Conditional(if_),
            _ => return None,
        };
        (leading, tail)
    };
    let statements = body_statements
        .iter()
        .map(|statement| scalar_statement(statement))
        .collect::<Option<Vec<_>>>()?;
    Some((statements, tail))
}

pub(super) fn scalar_expression_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    match expression {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value).is_some(),
        Expr::BoolLiteral(literal) => matches!(literal.value.as_str(), "true" | "false"),
        Expr::Identifier(identifier) => resolved.local_symbol_for_identifier(identifier).is_some(),
        Expr::Member(member) => super::projections::scalar_field_is_supported(
            member,
            SemanticInputs {
                resolved,
                resolved_sources,
                typed_hir,
            },
        ),
        Expr::Index(index) => {
            known_expression_type(expression, typed_hir)
                .and_then(|ty| scalar_type(ty, typed_hir))
                .is_some()
                && super::indexes::is_supported(
                    index,
                    SemanticInputs {
                        resolved,
                        resolved_sources,
                        typed_hir,
                    },
                )
        }
        Expr::Group(group) => {
            scalar_expression_is_supported(&group.expression, resolved, resolved_sources, typed_hir)
        }
        Expr::Call(call) => {
            scalar_value_call_is_supported(call, resolved, resolved_sources, typed_hir)
        }
        Expr::Force(force) => scalar_outcome_source_is_supported(
            &force.expression,
            resolved,
            resolved_sources,
            typed_hir,
            None,
        ),
        Expr::Propagate(propagate) => scalar_outcome_source_is_supported(
            &propagate.expression,
            resolved,
            resolved_sources,
            typed_hir,
            None,
        ),
        Expr::Otherwise(otherwise) => {
            scalar_outcome_source_is_supported(
                &otherwise.value,
                resolved,
                resolved_sources,
                typed_hir,
                None,
            ) && scalar_value_block_is_supported(
                &otherwise.fallback,
                resolved,
                resolved_sources,
                typed_hir,
            )
        }
        Expr::Catch(catch) => {
            catch_binding_is_supported(catch, resolved, typed_hir)
                && scalar_outcome_source_is_supported(
                    &catch.expression,
                    resolved,
                    resolved_sources,
                    typed_hir,
                    Some(crate::outcomes::OutcomeLayer::Fallible),
                )
                && scalar_value_block_is_supported(
                    &catch.catch_block,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::Unary(unary) => {
            let Some(operand_ty) = known_expression_type(&unary.operand, typed_hir) else {
                return false;
            };
            let Some(operand_scalar) = scalar_type(operand_ty, typed_hir) else {
                return false;
            };
            let operator_is_supported = match unary.operator {
                crate::ast::UnaryOperator::LogicalNot => operand_scalar == super::ScalarType::Bool,
                crate::ast::UnaryOperator::Negate => match operand_scalar {
                    super::ScalarType::I32 => true,
                    super::ScalarType::Integer(kind) => kind.is_signed(),
                    super::ScalarType::U8 | super::ScalarType::Usize | super::ScalarType::Bool => {
                        false
                    }
                },
                crate::ast::UnaryOperator::Move | crate::ast::UnaryOperator::Spread => false,
            };
            operator_is_supported
                && scalar_expression_is_supported(
                    &unary.operand,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::TypeConversion(conversion) => {
            let Some(source_ty) = known_expression_type(&conversion.expression, typed_hir) else {
                return false;
            };
            let Some(target_ty) = known_expression_type(expression, typed_hir) else {
                return false;
            };
            let Some(source_scalar) = scalar_type(source_ty, typed_hir) else {
                return false;
            };
            let Some(target_scalar) = scalar_type(target_ty, typed_hir) else {
                return false;
            };
            let checked_numeric_conversion = source_scalar == target_scalar
                || typed_hir
                    .conversion_plan(conversion.span)
                    .is_some_and(|plan| {
                        plan.kind == crate::typecheck::TypecheckConversionKind::LosslessInteger
                    });
            checked_numeric_conversion
                && scalar_expression_is_supported(
                    &conversion.expression,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::Binary(binary) => {
            (mir_binary_operator(binary.operator).is_some()
                || scalar_comparison_is_supported(binary, typed_hir)
                || scalar_logical_is_supported(binary, typed_hir))
                && scalar_expression_is_supported(
                    &binary.left,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
                && scalar_expression_is_supported(
                    &binary.right,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::If(if_) => {
            scalar_conditional_is_supported(if_, resolved, resolved_sources, typed_hir)
        }
        _ => false,
    }
}

pub(super) fn failure_value_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    if let Expr::Identifier(identifier) = expression.without_groups() {
        return semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .and_then(|symbol| semantic.typed_hir.binding_type_expr(symbol.id))
            .is_some_and(|ty| {
                matches!(
                    ty,
                    crate::ast::TypeExpr::Reference(reference)
                        if crate::builtin_types::BuiltinTypeOwner::from_reference_name(&reference.name)
                            == Some(crate::builtin_types::BuiltinTypeOwner::Error)
                )
            });
    }
    let Expr::Call(call) = expression.without_groups() else {
        return false;
    };
    let Some((owner, function)) = semantic.resolved.associated_function_for_call(call) else {
        return false;
    };
    if semantic.resolved.builtin_owner_for_symbol(owner)
        != Some(crate::builtin_types::BuiltinTypeOwner::Error)
        || semantic
            .typed_hir
            .associated_function_target(match call.callee.without_groups() {
                Expr::Member(member) => member.member_span,
                _ => return false,
            })
            .is_none()
        || function.signature.parameters.len() != 2
        || call.arguments.len() != 2
    {
        return false;
    }
    call.arguments.iter().all(|argument| {
        let Some(ty) = known_expression_type(argument, semantic.typed_hir) else {
            return false;
        };
        let representation = crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str);
        value_representation(ty, semantic) == Some(representation)
            && value_expression_is_supported(argument, representation, semantic)
    })
}

fn scalar_outcome_source_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    required_layer: Option<crate::outcomes::OutcomeLayer>,
) -> bool {
    if let Expr::Call(call) = expression.without_groups() {
        return match required_layer {
            Some(crate::outcomes::OutcomeLayer::Fallible) => {
                scalar_caught_call_is_supported(call, resolved, resolved_sources, typed_hir)
            }
            Some(crate::outcomes::OutcomeLayer::Optional) => false,
            None => {
                scalar_handled_call_is_supported(call, resolved, resolved_sources, typed_hir)
                    || scalar_outcome_call_is_supported(call, resolved, resolved_sources, typed_hir)
            }
        };
    }
    let Some(type_expr) =
        known_expression_type(expression, typed_hir).and_then(|ty| typed_hir.type_expr_by_id(ty))
    else {
        return false;
    };
    let shape = crate::outcomes::outcome_shape_with_resolver(type_expr, resolved, |source| {
        resolved_sources.get(&source).copied()
    });
    let [layer] = shape.layers.as_slice() else {
        return false;
    };
    required_layer.is_none_or(|required| required == *layer)
        && typed_hir
            .type_id(&shape.payload)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
        && match expression.without_groups() {
            Expr::Identifier(identifier) => {
                resolved.local_symbol_for_identifier(identifier).is_some()
            }
            _ => value_expression_is_supported(
                expression,
                crate::mir::ValueRepresentation::Aggregate,
                SemanticInputs {
                    resolved,
                    resolved_sources,
                    typed_hir,
                },
            ),
        }
}

fn catch_binding_is_supported(
    catch: &crate::ast::CatchExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    let crate::ast::CatchBinding::Named { span, .. } = catch.binding else {
        return true;
    };
    let Some(symbol) = resolved.local_symbol_id_at_name_span(span) else {
        return false;
    };
    if !typed_hir.binding_type_expr(symbol).is_some_and(
        |ty| matches!(ty, crate::ast::TypeExpr::Reference(reference) if reference.name == "error"),
    ) {
        return false;
    }
    let mut allowed_field_bases = std::collections::HashSet::new();
    crate::ast::visit_block_expressions_without_nested_closures(
        &catch.catch_block,
        &mut |expression| {
            let Expr::Member(member) = expression else {
                return;
            };
            let Expr::Identifier(base) = member.object.without_groups() else {
                return;
            };
            if resolved
                .local_symbol_for_identifier(base)
                .is_some_and(|candidate| candidate.id == symbol)
                && crate::builtin_types::BuiltinErrorField::from_source_name(&member.member)
                    .is_some()
            {
                allowed_field_bases.insert(base.span);
            }
        },
    );
    let mut used_outside_field = false;
    crate::ast::visit_block_expressions_without_nested_closures(
        &catch.catch_block,
        &mut |expression| {
            if let Expr::Identifier(identifier) = expression
                && resolved
                    .local_symbol_for_identifier(identifier)
                    .is_some_and(|candidate| candidate.id == symbol)
                && !allowed_field_bases.contains(&identifier.span)
            {
                used_outside_field = true;
            }
        },
    );
    !used_outside_field
}

fn scalar_logical_is_supported(binary: &crate::ast::BinaryExpr, typed_hir: &TypedHir) -> bool {
    matches!(
        binary.operator,
        crate::ast::BinaryOperator::LogicalAnd | crate::ast::BinaryOperator::LogicalOr
    ) && known_expression_type(&binary.left, typed_hir).and_then(|ty| scalar_type(ty, typed_hir))
        == Some(super::ScalarType::Bool)
        && known_expression_type(&binary.right, typed_hir).and_then(|ty| scalar_type(ty, typed_hir))
            == Some(super::ScalarType::Bool)
}

fn scalar_outcome_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                shape.layers.as_slice() == [crate::outcomes::OutcomeLayer::Fallible]
                    && typed_hir
                        .type_id(&shape.payload)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        .is_some()
            })
}

fn scalar_handled_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                matches!(
                    shape.layers.as_slice(),
                    [crate::outcomes::OutcomeLayer::Optional]
                        | [crate::outcomes::OutcomeLayer::Fallible]
                ) && typed_hir
                    .type_id(&shape.payload)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    .is_some()
            })
}

fn scalar_caught_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                shape.layers.as_slice() == [crate::outcomes::OutcomeLayer::Fallible]
                    && typed_hir
                        .type_id(&shape.payload)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        .is_some()
            })
}

pub(super) fn intrinsic_expression_type(
    span: crate::source::ByteSpan,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    let PartialSemantic::Known(ty) = typed_hir.expression(span)?.ty else {
        return None;
    };
    Some(ty)
}

fn scalar_value_block_is_supported(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let Some((statements, tail)) = scalar_body_parts(block) else {
        return false;
    };
    statements.iter().all(|statement| {
        statement.is_supported_in_context(resolved, resolved_sources, typed_hir, false)
    }) && tail.is_supported(SemanticInputs {
        resolved,
        resolved_sources,
        typed_hir,
    })
}

fn scalar_value_block_result_type(
    block: &Block,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    scalar_body_parts(block)?.1.result_type(typed_hir)
}

fn scalar_conditional_is_supported(
    if_: &IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_expression_is_supported(&if_.condition, resolved, resolved_sources, typed_hir)
        && scalar_value_block_is_supported(&if_.then_block, resolved, resolved_sources, typed_hir)
        && if_.else_block.as_ref().is_some_and(|block| {
            scalar_value_block_is_supported(block, resolved, resolved_sources, typed_hir)
        })
}

pub(super) fn value_conditional_is_supported(
    if_: &IfStmt,
    representation: crate::mir::ValueRepresentation,
    semantic: SemanticInputs<'_>,
) -> bool {
    scalar_expression_is_supported(
        &if_.condition,
        semantic.resolved,
        semantic.resolved_sources,
        semantic.typed_hir,
    ) && value_block_is_supported(&if_.then_block, representation, semantic)
        && if_
            .else_block
            .as_ref()
            .is_some_and(|block| value_block_is_supported(block, representation, semantic))
}

fn value_block_is_supported(
    block: &Block,
    representation: crate::mir::ValueRepresentation,
    semantic: SemanticInputs<'_>,
) -> bool {
    let Some((statements, tail)) = scalar_body_parts(block) else {
        return false;
    };
    if tail.expression().is_some_and(|expression| {
        semantic
            .typed_hir
            .expression(expression.span())
            .is_some_and(|typed| typed.diverges)
    }) {
        return false;
    }
    let statements_supported = statements.iter().all(|statement| {
        statement.is_supported_in_context(
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
            false,
        )
    });
    let tail_representation = tail
        .result_type(semantic.typed_hir)
        .and_then(|ty| value_representation(ty, semantic));
    if tail.is_explicit_return() && tail_representation != Some(representation) {
        return statements_supported
            && (tail.is_supported(semantic)
                || tail
                    .expression()
                    .is_some_and(|expression| failure_value_is_supported(expression, semantic)));
    }
    statements_supported
        && tail_representation == Some(representation)
        && (tail.expression().is_some_and(|expression| {
            value_expression_is_supported(expression, representation, semantic)
        }) || tail.conditional().is_some_and(|conditional| {
            value_conditional_is_supported(conditional, representation, semantic)
        }))
}

fn scalar_comparison_is_supported(binary: &crate::ast::BinaryExpr, typed_hir: &TypedHir) -> bool {
    let Some(operator) = mir_comparison_operator(binary.operator) else {
        return false;
    };
    if let Some(plan) = typed_hir.comparison_plan(binary.operator_span)
        && (plan.method.is_some()
            || plan.left_conversion.is_some()
            || plan.right_conversion.is_some())
    {
        return false;
    }
    let Some(left_ty) = known_expression_type(&binary.left, typed_hir) else {
        return false;
    };
    let Some(right_ty) = known_expression_type(&binary.right, typed_hir) else {
        return false;
    };
    let Some(left) = scalar_type(left_ty, typed_hir) else {
        return false;
    };
    let Some(right) = scalar_type(right_ty, typed_hir) else {
        return false;
    };
    left_ty == right_ty
        && left == right
        && (!matches!(left, super::ScalarType::Bool)
            || matches!(
                operator,
                ComparisonOperator::Equal | ComparisonOperator::NotEqual
            ))
}

pub(super) fn known_expression_type(
    expression: &Expr,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    effective_expression_type(expression.span(), typed_hir)
}

pub(super) fn value_representation(
    ty: crate::semantic::TyId,
    semantic: SemanticInputs<'_>,
) -> Option<crate::mir::ValueRepresentation> {
    if let Some(scalar) = scalar_type(ty, semantic.typed_hir) {
        return Some(crate::mir::ValueRepresentation::Scalar(scalar));
    }
    let ty = semantic.typed_hir.type_expr_by_id(ty)?;
    match crate::abi::abi_value_from_type_expr_with_resolver(ty, semantic.resolved, |source| {
        semantic.resolver_for(source)
    })
    .ok()?
    .ty
    {
        crate::abi::AbiType::StrView => Some(crate::mir::ValueRepresentation::View(
            crate::mir::ViewKind::Str,
        )),
        crate::abi::AbiType::Struct(_)
        | crate::abi::AbiType::Array { .. }
        | crate::abi::AbiType::Enum(_)
        | crate::abi::AbiType::Outcome { .. } => Some(crate::mir::ValueRepresentation::Aggregate),
        _ => None,
    }
}

pub(super) fn value_expression_is_supported(
    expression: &Expr,
    representation: crate::mir::ValueRepresentation,
    semantic: SemanticInputs<'_>,
) -> bool {
    match representation {
        crate::mir::ValueRepresentation::Scalar(_) => scalar_expression_is_supported(
            expression,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        ),
        crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => match expression {
            Expr::StringLiteral(literal) => {
                crate::literals::decode_string_literal_bytes(&literal.value).is_ok()
            }
            Expr::Identifier(identifier) => {
                semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .and_then(|symbol| semantic.typed_hir.binding_type_expr(symbol.id))
                    .and_then(|ty| semantic.typed_hir.type_id(ty))
                    .and_then(|ty| value_representation(ty, semantic))
                    == Some(representation)
            }
            Expr::Group(group) => {
                value_expression_is_supported(&group.expression, representation, semantic)
            }
            Expr::Member(member) => super::projections::error_field_is_supported(member, semantic),
            Expr::Call(call) => {
                scalar_call_shape_is_supported(
                    call,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                ) && intrinsic_expression_type(call.span, semantic.typed_hir)
                    .and_then(|ty| value_representation(ty, semantic))
                    == Some(representation)
            }
            _ => false,
        },
        crate::mir::ValueRepresentation::Aggregate => match expression.without_groups() {
            Expr::StructLiteral(_) | Expr::ArrayLiteral(_) | Expr::Member(_) | Expr::Closure(_) => {
                super::aggregates::literal_is_supported(expression, semantic)
            }
            Expr::Call(call) => aggregate_value_call_is_supported(
                call,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
            Expr::Identifier(_) | Expr::Unary(_) => stored_outcome_operand_is_supported(
                expression,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
            Expr::Force(force) => stored_outcome_projection_is_supported(
                expression,
                &force.expression,
                representation,
                semantic,
            ),
            Expr::Propagate(propagate) => stored_outcome_projection_is_supported(
                expression,
                &propagate.expression,
                representation,
                semantic,
            ),
            Expr::Otherwise(otherwise) => {
                aggregate_outcome_source_is_supported(&otherwise.value, None, semantic)
                    && value_block_is_supported(&otherwise.fallback, representation, semantic)
            }
            Expr::Catch(catch) => {
                catch_binding_is_supported(catch, semantic.resolved, semantic.typed_hir)
                    && aggregate_outcome_source_is_supported(
                        &catch.expression,
                        Some(crate::outcomes::OutcomeLayer::Fallible),
                        semantic,
                    )
                    && value_block_is_supported(&catch.catch_block, representation, semantic)
            }
            _ => false,
        },
        crate::mir::ValueRepresentation::Borrow | crate::mir::ValueRepresentation::Error => false,
    }
}

fn aggregate_outcome_source_is_supported(
    expression: &Expr,
    required_layer: Option<crate::outcomes::OutcomeLayer>,
    semantic: SemanticInputs<'_>,
) -> bool {
    let Some(type_expr) = known_expression_type(expression, semantic.typed_hir)
        .and_then(|ty| semantic.typed_hir.type_expr_by_id(ty))
    else {
        return false;
    };
    let shape =
        crate::outcomes::outcome_shape_with_resolver(type_expr, semantic.resolved, |source| {
            semantic.resolver_for(source)
        });
    let Some(layer) = shape.layers.first() else {
        return false;
    };
    if required_layer.is_some_and(|required| required != *layer) {
        return false;
    }
    match expression.without_groups() {
        Expr::Identifier(_) | Expr::Unary(_) => stored_outcome_operand_is_supported(
            expression,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        ),
        Expr::Call(call) => scalar_call_shape_is_supported(
            call,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        ),
        _ => value_expression_is_supported(
            expression,
            crate::mir::ValueRepresentation::Aggregate,
            semantic,
        ),
    }
}

fn stored_outcome_projection_is_supported(
    expression: &Expr,
    source: &Expr,
    result_representation: crate::mir::ValueRepresentation,
    semantic: SemanticInputs<'_>,
) -> bool {
    let Expr::Identifier(identifier) = source.without_groups() else {
        return false;
    };
    let Some(source_ty) = semantic
        .resolved
        .local_symbol_for_identifier(identifier)
        .and_then(|symbol| semantic.typed_hir.binding_type_expr(symbol.id))
    else {
        return false;
    };
    let shape =
        crate::outcomes::outcome_shape_with_resolver(source_ty, semantic.resolved, |source| {
            semantic.resolver_for(source)
        });
    !shape.layers.is_empty()
        && known_expression_type(expression, semantic.typed_hir)
            .and_then(|ty| value_representation(ty, semantic))
            == Some(result_representation)
}

fn stored_outcome_operand_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    if !aggregate_operand_is_supported(expression, resolved, resolved_sources, typed_hir) {
        return false;
    }
    let expression = match expression.without_groups() {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            unary.operand.without_groups()
        }
        expression => expression,
    };
    let Expr::Identifier(identifier) = expression else {
        return false;
    };
    resolved
        .local_symbol_for_identifier(identifier)
        .and_then(|symbol| typed_hir.binding_type_expr(symbol.id))
        .is_some_and(|ty| {
            matches!(
                crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    resolved_sources.get(&source).copied()
                })
                .map(|value| value.ty),
                Ok(crate::abi::AbiType::Outcome { .. })
            )
        })
}

fn effective_expression_type(
    span: crate::source::ByteSpan,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    let expression = typed_hir.expression(span)?;
    if let Some(ty) = expression.contextual_ty {
        return Some(ty);
    }
    let PartialSemantic::Known(ty) = expression.ty else {
        return None;
    };
    Some(ty)
}

pub(super) fn binding_scalar_type(
    symbol: LocalSymbolId,
    typed_hir: &TypedHir,
) -> Option<super::ScalarType> {
    let ty = typed_hir.binding_type_expr(symbol)?;
    scalar_type(typed_hir.type_id(ty)?, typed_hir)
}

pub(super) fn scalar_type(
    ty: crate::semantic::TyId,
    typed_hir: &TypedHir,
) -> Option<super::ScalarType> {
    match typed_hir.scalar_type(ty)? {
        CheckedScalarType::Integer(crate::integer::IntegerType::I32) => {
            Some(super::ScalarType::I32)
        }
        CheckedScalarType::Integer(crate::integer::IntegerType::U8) => Some(super::ScalarType::U8),
        CheckedScalarType::Integer(crate::integer::IntegerType::Usize) => {
            Some(super::ScalarType::Usize)
        }
        CheckedScalarType::Bool => Some(super::ScalarType::Bool),
        CheckedScalarType::Integer(kind) => Some(super::ScalarType::Integer(kind)),
    }
}
