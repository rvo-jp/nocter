use super::*;

pub(super) fn lower_error_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };

    let payload_context = context.with_reserved_error_local_abi_words();
    let Some(payload) = lower_error_payload(
        &statement.initializer,
        resolved,
        root_source,
        Some(&payload_context),
    )?
    else {
        return Ok(None);
    };

    let (code, message) = context.define_error_local(statement.name.clone())?;
    Ok(Some(payload.into_store_instructions(code, message)))
}

pub(super) fn lower_i32_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_i32_local_location()?;
    let instructions =
        lower_i32_expression_to_location(&statement.initializer, destination, context)?;
    context.define_i32_local(statement.name.clone());
    Ok(instructions)
}

pub(super) fn lower_u8_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_u8_local_location()?;
    let instructions =
        lower_u8_expression_to_location(&statement.initializer, destination, context)?;
    context.define_u8_local(statement.name.clone());
    Ok(instructions)
}

pub(super) fn lower_usize_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_usize_local_location()?;
    let instructions =
        lower_usize_expression_to_location(&statement.initializer, destination, context)?;
    context.define_usize_local(statement.name.clone());
    Ok(instructions)
}

pub(super) fn lower_borrow_local_binding(
    statement: &BindingStmt,
    is_readwrite: bool,
    inner: Type,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_usize_local_location()?;
    let borrow_type = Type::Borrow {
        is_readwrite,
        inner: Box::new(inner.clone()),
    };
    if let Some(instructions) = lower_stored_optional_otherwise(
        &statement.initializer,
        ComposedOutcomeDestination::Borrow(destination),
        context,
        |expression, context| {
            lower_borrow_expression_to_location(expression, destination, &borrow_type, context)
        },
        "IR can only lower stored borrow `otherwise` fallbacks that produce a matching borrow or exit",
    )? {
        context.define_borrow_local(statement.name.clone(), is_readwrite, inner);
        return Ok(instructions);
    }
    let instructions = lower_borrow_expression_to_location(
        &statement.initializer,
        destination,
        &borrow_type,
        context,
    )?;
    context.define_borrow_local(statement.name.clone(), is_readwrite, inner);
    Ok(instructions)
}

pub(super) fn lower_bool_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_bool_local_location()?;
    let instructions =
        lower_bool_expression_to_location(&statement.initializer, destination, context, "E8008")?;
    context.define_bool_local(statement.name.clone());
    Ok(instructions)
}

pub(super) fn lower_str_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_str_local_location()?;
    let instructions =
        lower_str_expression_to_location(&statement.initializer, destination, context)?;
    context.define_str_local(statement.name.clone());
    Ok(instructions)
}

pub(super) fn lower_slice_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    info: SliceTypeInfo,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_slice_local_location()?;
    let instructions =
        lower_slice_expression_to_location(&statement.initializer, destination, context)?;
    context.define_slice_local(statement.name.clone(), info.element_kind, info.element_type);
    Ok(instructions)
}

pub(super) fn scalar_binding_kind(
    statement: &BindingStmt,
    context: &LoweringContext,
) -> Result<ScalarBindingKind, Vec<Diagnostic>> {
    match &statement.ty {
        Some(ty) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_binding_diagnostic(
                    "native lowering cannot lower annotated local bindings without resolved type information",
                ));
            };
            let ty = context.specialize_type_expr(ty);
            match parameter_type_from_type_expr_with_resolver(&ty, resolved, |_| Some(resolved)) {
                Some(Type::I32) => Ok(ScalarBindingKind::I32),
                Some(Type::U8) => Ok(ScalarBindingKind::U8),
                Some(Type::Usize) => Ok(ScalarBindingKind::Usize),
                Some(Type::Bool) => Ok(ScalarBindingKind::Bool),
                Some(Type::Str) => Ok(ScalarBindingKind::Str),
                Some(Type::Slice { .. }) => Ok(ScalarBindingKind::Slice(
                    slice_type_info_from_type_expr(&ty, context),
                )),
                Some(Type::Borrow {
                    is_readwrite,
                    inner,
                }) => Ok(ScalarBindingKind::Borrow {
                    is_readwrite,
                    inner: *inner,
                }),
                _ => Err(unsupported_binding_diagnostic(
                    "IR lowering can only represent this annotation as a supported scalar, borrow, or view local",
                )),
            }
        }
        None => {
            if let Some(kind) = context.binding_scalar_view_kind(statement.name_span) {
                return Ok(scalar_binding_kind_from_typecheck_kind(
                    kind,
                    slice_type_info_from_expression(&statement.initializer, context),
                ));
            }
            if let Some(ty) = context.binding_type_expr(statement.name_span)
                && let Some((_root_source, resolved)) = context.resolved_calls()
                && let Some(Type::Borrow {
                    is_readwrite,
                    inner,
                }) = parameter_type_from_type_expr_with_resolver(&ty, resolved, |source| {
                    context.resolved_source(source)
                })
            {
                return Ok(ScalarBindingKind::Borrow {
                    is_readwrite,
                    inner: *inner,
                });
            }
            Ok(
                expression_is_lowerable_bool_binding(&statement.initializer, context)
                    .then_some(ScalarBindingKind::Bool)
                    .or_else(|| {
                        expression_is_bool_returning_call(&statement.initializer, context)
                            .then_some(ScalarBindingKind::Bool)
                    })
                    .or_else(|| expression_scalar_binding_kind(&statement.initializer, context))
                    .unwrap_or(ScalarBindingKind::I32),
            )
        }
    }
}

pub(super) fn scalar_binding_kind_from_typecheck_kind(
    kind: TypecheckScalarViewKind,
    slice_info: Option<SliceTypeInfo>,
) -> ScalarBindingKind {
    match kind {
        TypecheckScalarViewKind::I32 => ScalarBindingKind::I32,
        TypecheckScalarViewKind::U8 => ScalarBindingKind::U8,
        TypecheckScalarViewKind::Usize => ScalarBindingKind::Usize,
        TypecheckScalarViewKind::Bool => ScalarBindingKind::Bool,
        TypecheckScalarViewKind::Str => ScalarBindingKind::Str,
        TypecheckScalarViewKind::Slice(element_kind) => {
            ScalarBindingKind::Slice(slice_info.unwrap_or(SliceTypeInfo {
                element_kind,
                element_type: None,
            }))
        }
    }
}

pub(super) fn expression_scalar_binding_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match unwrap_group(expression) {
        Expr::Call(call) => call_return_scalar_binding_kind(call, context),
        Expr::Propagate(propagation) => {
            propagated_expression_scalar_binding_kind(&propagation.expression, context)
        }
        Expr::Force(force) => {
            outcome_call_payload_scalar_binding_kind(unwrap_group(&force.expression), context)
        }
        Expr::Catch(catch) => {
            outcome_call_payload_scalar_binding_kind(unwrap_group(&catch.expression), context)
        }
        Expr::Member(member) if context.payloadless_enum_variant_tag(member).is_some() => {
            Some(ScalarBindingKind::U8)
        }
        _ => None,
    }
}

fn propagated_expression_scalar_binding_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    if let Expr::Identifier(identifier) = unwrap_group(expression)
        && let Some(local) = context.outcome_local(&identifier.name)
    {
        return scalar_binding_kind_from_type(&local.payload_type);
    }
    if let Some(kind) = outcome_call_payload_scalar_binding_kind(unwrap_group(expression), context)
    {
        return Some(kind);
    }

    let ty = context.expression_type_expr(expression.span())?;
    let (_, resolved) = context.resolved_calls()?;
    let shape =
        outcome_shape_with_resolver(&ty, resolved, |source| context.resolved_source(source));
    let payload_type = context.ir_type_for_type_expr(&shape.payload)?;
    scalar_binding_kind_from_type(&payload_type)
}

pub(super) fn call_return_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    if let Some(kind) = primitive_call_scalar_binding_kind(call, context) {
        return Some(kind);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    scalar_binding_kind_from_call_return_type(call, context.call_return_type(&target)?, context)
}

pub(super) fn outcome_call_payload_scalar_binding_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    let Expr::Call(call) = expression else {
        return None;
    };
    if let Some(kind) = primitive_call_fallible_success_scalar_binding_kind(call, context) {
        return Some(kind);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    let (_, success) = context.call_return_type(&target)?.single_outcome()?;
    scalar_binding_kind_from_call_success_type(call, success, context)
}

pub(super) fn primitive_call_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match context.primitive_name_for_call(call)? {
        "addr" | "from_addr" | "from_ref" | "from_ref_mut" | "pointee_size" => {
            Some(ScalarBindingKind::Usize)
        }
        "str_from_raw_parts" | "str_subview_unchecked" => Some(ScalarBindingKind::Str),
        "bytes_from_str" => Some(ScalarBindingKind::Slice(slice_type_info_from_kind(
            TypecheckSliceElementKind::U8,
        ))),
        "slice_from_raw_parts"
        | "slice_from_raw_parts_mut"
        | "slice_from_raw_parts_value"
        | "slice_from_raw_parts_value_mut" => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_return(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_return_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => None,
    }
}

pub(super) fn primitive_call_fallible_success_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match context.primitive_name_for_call(call)? {
        "open_read_raw" | "create_raw" | "append_raw" => Some(ScalarBindingKind::I32),
        "read_bytes_raw" => Some(ScalarBindingKind::Usize),
        _ => None,
    }
}

pub(super) fn scalar_binding_kind_from_type(ty: &Type) -> Option<ScalarBindingKind> {
    match ty {
        Type::I32 => Some(ScalarBindingKind::I32),
        Type::U8 => Some(ScalarBindingKind::U8),
        Type::Usize => Some(ScalarBindingKind::Usize),
        Type::Bool => Some(ScalarBindingKind::Bool),
        Type::Str => Some(ScalarBindingKind::Str),
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(slice_type_info_from_kind(
            TypecheckSliceElementKind::Other,
        ))),
        Type::Borrow {
            is_readwrite,
            inner,
        } => Some(ScalarBindingKind::Borrow {
            is_readwrite: *is_readwrite,
            inner: inner.as_ref().clone(),
        }),
        _ => None,
    }
}

pub(super) fn scalar_binding_kind_from_call_return_type(
    call: &CallExpr,
    ty: &Type,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match ty {
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_return(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_return_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => scalar_binding_kind_from_type(ty),
    }
}

pub(super) fn scalar_binding_kind_from_call_success_type(
    call: &CallExpr,
    ty: &Type,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match ty {
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_success(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_success_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => scalar_binding_kind_from_type(ty),
    }
}

pub(super) fn expression_is_bool_returning_call(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Call(call) => {
            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return false;
            };
            context.call_return_type(&target) == Some(&Type::Bool)
        }
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_bool_returning_call(&unary.operand, context)
        }
        Expr::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
            ) =>
        {
            expression_is_bool_returning_call(&binary.left, context)
                && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_lowerable_bool_binding(&binary.left, context)
                    && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_bool_returning_call(&binary.left, context)
                    && expression_is_lowerable_bool_binding(&binary.right, context)
        }
        Expr::Group(group) => expression_is_bool_returning_call(&group.expression, context),
        _ => false,
    }
}

pub(super) enum ScalarBindingKind {
    I32,
    U8,
    Usize,
    Borrow { is_readwrite: bool, inner: Type },
    Bool,
    Str,
    Slice(SliceTypeInfo),
}

impl ScalarBindingKind {
    pub(super) fn abi_word_count(&self) -> usize {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Borrow { .. } | Self::Bool => 1,
            Self::Str | Self::Slice(_) => 2,
        }
    }
}
